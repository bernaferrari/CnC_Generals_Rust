//! Host combat `impl GameLogic` — `ocl_and_scud`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

#[cfg(test)]
mod receiving_debris_ocl_owner_tests;
mod nuke_cannon_overlord;
mod technical_gattling_dragon;
mod marauder_scorpion_shells;
#[cfg(test)]
mod tests;

/// Results already created by an OCL stay committed if a later nugget fails.
/// Receipts retain the birth identity needed by reentrant callers.
pub(crate) struct OclExecutionOutcome {
    pub(crate) created: Vec<(ObjectId, (u64, u64))>,
    pub(crate) error: Option<gamelogic::GameLogicError>,
}

impl OclExecutionOutcome {
    fn success(created: Vec<(ObjectId, (u64, u64))>) -> Self {
        Self {
            created,
            error: None,
        }
    }

    fn failure(
        created: Vec<(ObjectId, (u64, u64))>,
        error: gamelogic::GameLogicError,
    ) -> Self {
        Self {
            created,
            error: Some(error),
        }
    }
}

/// Whether this host bridge can reproduce a parsed generic OCL nugget.
/// Unsupported physics/container/debris nuggets are skipped individually —
/// C++ `ObjectCreationList::createInternal` still runs every later nugget.
fn supports_host_weapon_generic_ocl(
    nugget: &gamelogic::object_creation_list::GenericObjectCreationNugget,
) -> bool {
    use gamelogic::object_creation_list::DebrisDisposition;
    let unsupported_disposition = DebrisDisposition::SEND_IT_FLYING
        | DebrisDisposition::SEND_IT_UP
        | DebrisDisposition::SEND_IT_OUT
        | DebrisDisposition::RANDOM_FORCE
        | DebrisDisposition::FLOATING
        | DebrisDisposition::WHIRLING;

    nugget.name_are_objects
        && !nugget.names.is_empty()
        && nugget.debris_to_generate > 0
        && !nugget.disposition.has(unsupported_disposition)
        && nugget.put_in_container.trim().is_empty()
        && !nugget.ignore_primary_obstacle
        && !nugget.spread_formation
        && !nugget.dies_on_bad_land
        && !nugget.requires_live_player
}

fn unsupported_host_ocl_nugget_name(
    nugget: &dyn gamelogic::object_creation_list::ObjectCreationNugget,
) -> &'static str {
    let nugget = nugget.as_any();
    use gamelogic::object_creation_list::advanced_nuggets::{
        ApplyRandomForceNugget, AttackNugget, DeliverPayloadNugget, FireWeaponNugget,
    };
    if nugget.is::<FireWeaponNugget>() {
        "FireWeapon"
    } else if nugget.is::<DeliverPayloadNugget>() {
        "DeliverPayload"
    } else if nugget.is::<AttackNugget>() {
        "Attack"
    } else if nugget.is::<ApplyRandomForceNugget>() {
        "ApplyRandomForce"
    } else {
        "other"
    }
}

fn host_ocl_disposition_bits(
    disposition: gamelogic::object_creation_list::DebrisDisposition,
) -> u32 {
    use gamelogic::object_creation_list::DebrisDisposition as D;
    [
        D::LIKE_EXISTING,
        D::ON_GROUND_ALIGNED,
        D::SEND_IT_FLYING,
        D::SEND_IT_UP,
        D::SEND_IT_OUT,
        D::RANDOM_FORCE,
        D::FLOATING,
        D::INHERIT_VELOCITY,
        D::WHIRLING,
    ]
    .into_iter()
    .filter(|flag| disposition.has(*flag))
    .fold(0, |bits, flag| bits | flag)
}

fn usable_ocl_particle_sys_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then_some(name)
}

/// C++ `getPrimaryDamageRadius` residual for FireFX / DetonationFX overrideRadius.
fn host_fire_fx_override_radius(shooter: Option<&Object>) -> f32 {
    shooter
        .and_then(|o| o.weapon_name_for_slot(0))
        .map(crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name)
        .unwrap_or(0.0)
}

impl GameLogic {
    /// Execute a parsed `Weapon.ini` OCL at a source origin.
    ///
    /// C++ `Weapon::fireWeaponTemplate` calls `ObjectCreationList::create`
    /// with the firing object for `FireOCL`, and with the live missile object
    /// for `ProjectileDetonationOCL` (`Weapon.cpp:943-949`,
    /// `MissileAIUpdate.cpp:364-381`).  The host has no simulation `Object`
    /// for every visual projectile, so the caller supplies that object's
    /// frozen team/rank and its fire/impact origin.  References are resolved
    /// only through the receiving session's admitted OCL and object content;
    /// unresolved lists and templates do not select another world's assets.
    /// Object creation runs synchronously against this instance's templates,
    /// RNG, and object ownership.
    pub fn execute_parsed_weapon_ocl_at(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
    ) -> Vec<ObjectId> {
        self.execute_parsed_weapon_ocl_at_with_effects(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_parsed_weapon_ocl_at_with_effects(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        self.execute_parsed_weapon_ocl_at_with_interpreter_and_effects(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            None,
            presentation_effects,
        )
    }

    pub(crate) fn execute_parsed_weapon_ocl_at_with_interpreter_and_effects(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        let outcome = self.execute_parsed_weapon_ocl_with_births(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            interpreter,
            presentation_effects,
        );
        if let Some(error) = outcome.error {
            log::error!("{error}");
        }
        outcome
            .created
            .into_iter()
            .filter_map(|(id, (generation, epoch))| {
                self.spawn_identity_matches(id, generation, epoch).then_some(id)
            })
            .collect()
    }

    pub(crate) fn execute_parsed_weapon_ocl_with_births(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        self.execute_parsed_weapon_ocl_with_secondary_and_births(
            ocl_name, source_id, source_team, source_veterancy, source_orientation,
            source_velocity, origin, None, interpreter, presentation_effects,
        )
    }

    /// Execute with the live secondary source whose position the OCL receives.
    pub(crate) fn execute_parsed_weapon_ocl_with_secondary_and_births(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        secondary: Option<(ObjectId, &crate::assets::LiveDrawPlaybackIdentity)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        use gamelogic::object_creation_list::{
            DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget,
        };

        let ocl_name = ocl_name.trim();
        if source_id.is_none() || ocl_name.is_empty() || ocl_name.eq_ignore_ascii_case("None") {
            return OclExecutionOutcome::success(Vec::new());
        }
        let source_birth = source_id.and_then(|id| {
            self.objects.get(&id).map(|source| {
                (source.visual_object_generation, self.visual_world_epoch)
            })
        });
        let source_owner_player_id = source_id.and_then(|id| {
            self.objects
                .get(&id)
                .and_then(|source| source.owner_player_id)
        });

        let Some(ocl) = self.object_creation_list_definition(ocl_name) else {
            log::warn!(
                "receiving OCL `{ocl_name}` was not admitted for this GameLogic; no global fallback was used"
            );
            return OclExecutionOutcome::success(Vec::new());
        };

        // C++ ObjectCreationList.cpp:1524-1534 walks the nuggets in order.
        // Unsupported nuggets do not discard earlier results or later nuggets.
        // FireOCL after FireFX may run with a destroyed source; frozen team/pose
        // still create (hq-ye145). Only stop when a captured birth no longer
        // matches — that means a same-ID replacement, not a missing original.
        let mut created = Vec::new();
        for nugget in ocl.nuggets() {
            if let Some(birth) = source_birth {
                if !source_id.is_some_and(|id| self.spawn_identity_matches(id, birth.0, birth.1)) {
                    break;
                }
            }
            if let Some(fire) = nugget.as_any().downcast_ref::<
                gamelogic::object_creation_list::advanced_nuggets::FireWeaponNugget,
            >() {
                let position = secondary.as_ref().and_then(|(id, identity)| {
                    self.objects.get(id)
                        .filter(|object| self.live_draw_playback_identity(object) == **identity)
                        .map(|object| object.get_position())
                });
                if let (Some(source), Some(position), Some(weapon)) =
                    (source_id, position, fire.weapon.as_deref())
                {
                    self.fire_ocl_temporary_weapon_at(source, weapon, position, presentation_effects);
                }
                continue;
            }
            let Some(generic) = nugget
                .as_any()
                .downcast_ref::<GenericObjectCreationNugget>()
            else {
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported {} nugget; no Core OCL fallback was used",
                    unsupported_host_ocl_nugget_name(nugget.as_ref())
                );
                continue;
            };
            if generic.debris_to_generate == 0 {
                continue;
            }
            if generic.contain_inside_source_object
                && !source_id.is_some_and(|source_id| {
                    self.objects
                        .get(&source_id)
                        .is_some_and(|source| source.admits_ocl_portable_structure())
                })
            {
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported ContainInsideSourceObject container"
                );
                continue;
            }
            if generic.contain_inside_source_object
                && !generic.names.iter().all(|name| {
                    // Resolve this session's admitted payload definition
                    // before reading its KindOf (creation would do the same).
                    let name = name.trim();
                    self.ensure_host_spawn_template(name);
                    self.templates.get(name).is_some_and(|template| {
                        template.is_kind_of(crate::game_logic::KindOf::PortableStructure)
                    })
                })
            {
                // The ordinary TransportContain rider path is still
                // unsupported here; don't create-and-destroy its child.
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported non-portable ContainInsideSourceObject candidate"
                );
                continue;
            }
            if !generic.name_are_objects {
                let debris = self.execute_receiving_create_debris(
                    generic,
                    source_id.expect("validated above"),
                    source_birth.expect("source birth captured"),
                    source_team,
                    origin,
                    presentation_effects,
                );
                created.extend(debris.created);
                if let Some(error) = debris.error {
                    return OclExecutionOutcome::failure(created, error);
                }
                continue;
            }
            if !supports_host_weapon_generic_ocl(generic) {
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported GenericObjectCreationNugget configuration"
                );
                continue;
            }

            // `GenericObjectCreationNugget::create` checks source altitude
            // before spawning.  The host coordinate system is Y-up, whereas
            // C++ Coord3D is Z-up; terrain_height_at already accepts host X/Z.
            if generic.skip_if_significantly_airborne {
                let terrain_y = self.terrain_height_at(origin).unwrap_or(origin.y);
                if crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                    origin.y - terrain_y,
                ) {
                    continue;
                }
            }

            let count = usize::try_from(generic.debris_to_generate).unwrap_or_default();
            let upper_pick = match i32::try_from(generic.names.len().saturating_sub(1)) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for _ in 0..count {
                if !source_id.is_some_and(|id| {
                    source_birth
                        .is_some_and(|birth| self.spawn_identity_matches(id, birth.0, birth.1))
                }) {
                    return OclExecutionOutcome::success(created);
                }
                // C++ chooses once per created object from ObjectNames using
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

                // The receiving session admitted its authored templates before
                // the match. A missing name cannot be recovered from another
                // application's asset manager or a decorative fallback.
                if !self.templates.contains_key(template_name) {
                    log::warn!(
                        "receiving OCL `{ocl_name}` references unadmitted object template `{template_name}`"
                    );
                    continue;
                }

                // C++ `GenericObjectCreationNugget::doStuffToObj` transforms
                // `Offset` through the primary object's matrix before adding
                // it to the source position (ObjectCreationList.cpp:953-960).
                // `Coord3D` is (X, Y-horizontal, Z-up), whereas the host is
                // X/Z-horizontal with Y-up, so convert first and then apply
                // the frozen source yaw.  Leaving this unrotated puts every
                // directional FireOCL/debris spawn on the same world side of
                // its source regardless of facing.
                let local_offset = Vec3::new(generic.offset.x, generic.offset.z, generic.offset.y);
                let rotated_offset = glam::Mat3::from_rotation_y(source_orientation) * local_offset;
                let mut spawn_position = origin + rotated_offset;
                let on_ground = generic
                    .disposition
                    .has(DebrisDisposition::ON_GROUND_ALIGNED);
                if on_ground {
                    spawn_position.y = self
                        .terrain_height_at(spawn_position)
                        .unwrap_or(spawn_position.y);
                }

                // C++ ObjectCreationList.cpp:1302-1305
                // `sourceObj->getControllingPlayer()->getDefaultTeam()`.
                // `create_object` uses `unique_player_id_for_team`, which is
                // None in 2v2 same-faction, dropping the controlling player.
                let creation = self.create_object_for_owner_or_team_with_birth_checked(
                    template_name,
                    source_team,
                    source_owner_player_id,
                    spawn_position,
                    interpreter,
                    presentation_effects,
                );
                let (created_id, created_birth) = match creation {
                    Ok(Some(created)) => created,
                    Ok(None) => continue,
                    Err(error) => {
                        if let Some(committed) = error.committed {
                            created.push(committed);
                        }
                        return OclExecutionOutcome::failure(created, error.error);
                    }
                };
                // Registration is already committed, even if a later callback
                // removes this birth before the OCL finishes.
                created.push((created_id, created_birth));
                let birth_matches = |world: &Self| {
                    world.spawn_identity_matches(created_id, created_birth.0, created_birth.1)
                };
                if !birth_matches(self) {
                    continue;
                }
                if !source_id.is_some_and(|id| {
                    source_birth
                        .is_some_and(|birth| self.spawn_identity_matches(id, birth.0, birth.1))
                }) {
                    if generic.contain_inside_source_object && birth_matches(self) {
                        if let Err(error) = self.destroy_object_with_effects(
                            created_id,
                            presentation_effects,
                        ) {
                            return OclExecutionOutcome::failure(created, error);
                        }
                    }
                    return OclExecutionOutcome::success(created);
                }
                if let Some(object) = self.objects.get_mut(&created_id) {
                    object.producer_id = source_id;
                    if generic.disposition.has(DebrisDisposition::LIKE_EXISTING) {
                        object.set_orientation_with_partition_owner(
                            source_orientation,
                            &mut self.partition_manager,
                        );
                    }
                }
                // C++ ObjectCreationList.cpp:985-993: any created object with a
                // SlavedUpdate is enslaved to the source (startSlavedEffects ->
                // UNSELECTABLE), independent of upgrade/OCL spelling.
                if let (Some(source), Some(birth)) = (source_id, source_birth) {
                    if self
                        .objects
                        .get(&created_id)
                        .is_some_and(|object| object.thing().template.slaved_update_metadata.is_some())
                    {
                        self.enslave_owned_drone(created_id, source, birth);
                    }
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
                if !source_id.is_some_and(|id| {
                    source_birth
                        .is_some_and(|birth| self.spawn_identity_matches(id, birth.0, birth.1))
                }) {
                    if generic.contain_inside_source_object && birth_matches(self) {
                        if let Err(error) = self.destroy_object_with_effects(
                            created_id,
                            presentation_effects,
                        ) {
                            return OclExecutionOutcome::failure(created, error);
                        }
                    }
                    return OclExecutionOutcome::success(created);
                }
                let Some(object) = self.objects.get_mut(&created_id) else {
                    continue;
                };
                if generic.max_frames > 0 && object.native_lifecycle.is_some() {
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
                object.set_extra_friction(generic.extra_friction);
                if generic.disposition.has(
                    gamelogic::object_creation_list::DebrisDisposition::SEND_IT_FLYING
                        | gamelogic::object_creation_list::DebrisDisposition::SEND_IT_UP
                        | gamelogic::object_creation_list::DebrisDisposition::RANDOM_FORCE,
                ) {
                    object.set_extra_bounciness(generic.extra_bounciness);
                    object.set_allow_bouncing(true);
                }
                if !generic.bounce_sound.is_empty() {
                    object.set_bounce_sound(generic.bounce_sound.clone());
                }
                if generic.fade_in {
                    object.start_drawable_fade_in(generic.fade_frames, self.frame);
                } else if generic.fade_out {
                    object.start_drawable_fade_out(generic.fade_frames, self.frame);
                }
                drop(object);
                if inherit_name {
                    if let Some(sid) = source_id {
                        if !source_birth
                            .is_some_and(|birth| self.spawn_identity_matches(sid, birth.0, birth.1))
                        {
                            if generic.contain_inside_source_object && birth_matches(self) {
                                if let Err(error) = self.destroy_object_with_effects(
                                    created_id,
                                    presentation_effects,
                                )
                                {
                                    return OclExecutionOutcome::failure(created, error);
                                }
                            }
                            return OclExecutionOutcome::success(created);
                        }
                        // C++ ObjectCreationList.cpp:1005
                        // TheScriptEngine->transferObjectName(sourceObj->getName(), obj)
                        let _ = self.transfer_script_object_name(sid, created_id);
                    }
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
                    let admitted = source_id.is_some_and(|source_id| {
                        source_birth.is_some_and(|source_birth| {
                            birth_matches(self)
                                && self.admit_ocl_portable_structure(
                                    source_id,
                                    created_id,
                                    source_birth,
                                    created_birth,
                                )
                        })
                    });
                    if !admitted && birth_matches(self) {
                        // C++ reallyCreate returns firstObject even when
                        // doStuffToObj schedules that birth for destruction;
                        // retain the receipt, but destroy only that birth.
                        if let Err(error) = self.destroy_object_with_effects(
                            created_id,
                            presentation_effects,
                        ) {
                            return OclExecutionOutcome::failure(created, error);
                        }
                    }
                }
            }
        }
        OclExecutionOutcome::success(created)
    }

    /// Admit the C++ `ContainInsideSourceObject` branch for the native
    /// OverlordContain and HelixContain portable-structure slot only.
    fn admit_ocl_portable_structure(
        &mut self,
        source_id: ObjectId,
        child_id: ObjectId,
        source_birth: (u64, u64),
        child_birth: (u64, u64),
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
        self.attach_overlord_portable_occupant(source_id, child_id);
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

    /// Bounded receiving-session implementation of authored CreateDebris.
    /// This supports the live bridge transition slice (a required receiving
    /// GenericDebris template, per-item model-name selection, positive mass,
    /// optional receiving LifetimeUpdate range, and SEND_IT_FLYING disposition).
    /// Unsupported draw/physics options fail closed instead of substituting a
    /// model template or fallback geometry.
    fn execute_receiving_create_debris(
        &mut self,
        generic: &gamelogic::object_creation_list::GenericObjectCreationNugget,
        source_id: ObjectId,
        source_birth: (u64, u64),
        source_team: Team,
        origin: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        use gamelogic::object_creation_list::DebrisDisposition as D;
        use gamelogic::object_creation_list::StaticGameLODLevel;

        let supported = generic.debris_to_generate > 0
            && !generic.names.is_empty()
            && generic.mass > 0.0
            && generic.disposition.has(D::SEND_IT_FLYING)
            && !generic.disposition.has(
                D::LIKE_EXISTING
                    | D::ON_GROUND_ALIGNED
                    | D::SEND_IT_UP
                    | D::SEND_IT_OUT
                    | D::RANDOM_FORCE
                    | D::FLOATING
                    | D::INHERIT_VELOCITY
                    | D::WHIRLING,
            )
            && generic.put_in_container.trim().is_empty()
            && generic.particle_sys_name.trim().is_empty()
            && generic.anim_sets.is_empty()
            && generic.fx_final.is_none()
            && generic.min_lod_required == StaticGameLODLevel::Low
            && (generic.max_frames == 0 || generic.min_frames <= generic.max_frames)
            && !generic.fade_in
            && !generic.fade_out
            && !generic.spread_formation
            && !generic.contain_inside_source_object
            && !generic.ignore_primary_obstacle
            && !generic.skip_if_significantly_airborne
            && !generic.requires_live_player
            && !generic.dies_on_bad_land
            && !generic.orient_in_force_direction
            && generic.preserve_layer;
        if !supported {
            log::warn!(
                "receiving CreateDebris nugget is outside the GenericDebris bridge slice; no Core ThingFactory/GameLOD fallback was used"
            );
            return OclExecutionOutcome::success(Vec::new());
        }
        if !self.templates.contains_key("GenericDebris") {
            log::warn!(
                "receiving CreateDebris requires the admitted GenericDebris template; no fallback object geometry was created"
            );
            return OclExecutionOutcome::success(Vec::new());
        }
        let Some((owner_player_id, source_layer, source_matrix, source_orientation)) =
            self.objects.get(&source_id).map(|source| {
                (
                    source.owner_player_id,
                    source.pathfind_layer,
                    source.get_transform_matrix(),
                    source.get_orientation(),
                )
            })
        else {
            return OclExecutionOutcome::success(Vec::new());
        };
        let team = if owner_player_id.is_some() {
            source_team
        } else {
            Team::Neutral
        };
        self.ocl_create_debris_reg
            .record_plan(host_ocl_disposition_bits(generic.disposition));

        let mut created = Vec::new();
        let count = u32::try_from(generic.debris_to_generate).unwrap_or_default();
        for _ in 0..count {
            if !self.spawn_identity_matches(source_id, source_birth.0, source_birth.1) {
                break;
            }
            // C++ selects ModelNames before calling GameLODManager::isDebrisSkipped.
            let upper = i32::try_from(generic.names.len().saturating_sub(1)).unwrap_or(i32::MAX);
            let pick = usize::try_from(self.logic_random.next_int(0, upper))
                .unwrap_or_default()
                .min(generic.names.len() - 1);
            if self.ocl_create_debris_reg.is_debris_skipped() {
                continue;
            }
            let model_name = generic.names[pick].trim();
            if model_name.is_empty() || model_name.eq_ignore_ascii_case("none") {
                log::warn!("receiving CreateDebris selected an empty model name; item skipped");
                continue;
            }
            // C++ ObjectCreationList.cpp:953-960 transforms its Z-up Coord3D
            // Offset through the primary Object matrix before translating.
            // Thing's receiving transform matrix is in that same source basis;
            // transform the vector without applying the matrix translation.
            let local_offset = Vec3::new(generic.offset.x, generic.offset.y, generic.offset.z);
            let position = origin + source_matrix.transform_vector3(local_offset);
            let creation = self.create_object_for_owner_or_team_with_birth_checked(
                "GenericDebris",
                team,
                owner_player_id,
                position,
                None,
                presentation_effects,
            );
            let (id, birth) = match creation {
                Ok(Some(created)) => created,
                Ok(None) => continue,
                Err(error) => {
                    if let Some(committed) = error.committed {
                        created.push(committed);
                    }
                    return OclExecutionOutcome::failure(created, error.error);
                }
            };
            created.push((id, birth));
            if !self.spawn_identity_matches(id, birth.0, birth.1) {
                continue;
            }

            // C++ doStuffToObj sets producer and per-instance DebrisDraw state.
            // Keep model selection on the new Object; the shared template remains
            // unchanged for this and all other GameLogic owners.
            if let Some(object) = self.objects.get_mut(&id) {
                object.producer_id = Some(source_id);
            }
            if generic.preserve_layer && source_layer != 1 {
                self.set_owned_pathfinder_layer(id, source_layer);
            }
            if let Some(object) = self.objects.get_mut(&id) {
                // C++ doStuffToObj: override the first existing LifetimeUpdate
                // only when the nugget authored a positive maximum. The setter
                // consumes this receiving world's RNG after factory creation.
                if generic.max_frames > 0 {
                    let _ = object.set_native_lifetime_range(
                        self.frame,
                        crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                            minimum: generic.min_frames,
                            maximum: generic.max_frames,
                        },
                        &self.logic_random,
                    );
                }
                object.debris_draw = Some(crate::game_logic::object::visual::DebrisDrawState {
                    model_name: model_name.to_owned(),
                    anim_initial: String::new(),
                    anim_flying: String::new(),
                    anim_final: String::new(),
                    fx_final: None,
                    allow_model_color_change: generic.ok_to_change_model_color,
                    shadow_type: generic.shadow_type,
                });

                // C++ calls setInitialHealth before processing dispositions.
                // It consumes a random health percentage even when no body is
                // present, so retain the draw before the flight randoms.
                let health_percent = self
                    .logic_random
                    .next_real(generic.min_health, generic.max_health)
                    .clamp(0.0, 1.0);
                let target_health = object.body_initial_health() * health_percent;
                object.internal_change_health(target_health - object.health.current);

                // C++ SEND_IT_FLYING force and angular-rate draw order.
                // Mass and bounce properties are installed after initial
                // health, at the start of disposition handling.
                object.physics_mass = generic.mass;
                object.extra_bounciness = generic.extra_bounciness;
                object.extra_friction = generic.extra_friction;
                object.set_allow_bouncing(true);
                object.bounce_sound_name = generic.bounce_sound.clone();
                let spin = if generic.spin_rate >= 0.0 {
                    generic.spin_rate
                } else {
                    (std::f32::consts::PI / 32.0) * generic.disposition_intensity
                };
                let yaw_rate = if generic.yaw_rate >= 0.0 {
                    generic.yaw_rate
                } else {
                    spin
                };
                let roll_rate = if generic.roll_rate >= 0.0 {
                    generic.roll_rate
                } else {
                    spin
                };
                let pitch_rate = if generic.pitch_rate >= 0.0 {
                    generic.pitch_rate
                } else {
                    spin
                };
                object.shock_yaw_rate = self.logic_random.next_real(-yaw_rate, yaw_rate);
                object.shock_roll_rate = self.logic_random.next_real(-roll_rate, roll_rate);
                object.shock_pitch_rate = self.logic_random.next_real(-pitch_rate, pitch_rate);
                let horizontal = 4.0 * generic.disposition_intensity;
                let vertical = 3.0 * generic.disposition_intensity;
                let force_x = self.logic_random.next_real(-horizontal, horizontal);
                let force_y = self.logic_random.next_real(-horizontal, horizontal);
                let force_z = self.logic_random.next_real(vertical * 0.33, vertical);
                // Draw in C++ Coord3D x/y/z order, then map the vertical
                // component to this host's Y-up force vector.
                let force = Vec3::new(force_x, force_z, force_y);
                object.apply_physics_force(force, self.frame);
                object.set_orientation_with_partition_owner(
                    source_orientation,
                    &mut self.partition_manager,
                );
            }
            self.ocl_create_debris_reg
                .record_spawn(host_ocl_disposition_bits(generic.disposition));
        }
        OclExecutionOutcome::success(created)
    }

    /// Materialize FireFX and FireOCL events stamped when pending projectiles
    /// were drained. This is kept in GameLogic so both the normal host tick
    /// and GameWorld fire-spawn apply path use the same real object lifecycle.
    pub(crate) fn execute_pending_weapon_fire_ocls(&mut self) {
        self.execute_pending_weapon_fire_ocls_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_pending_weapon_fire_ocls_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let fire_ocls = self.combat_system.take_fire_ocl();
        for fire in fire_ocls {
            // C++ Weapon.cpp:889 wraps FireFX/muzzle in getDrawable(); FireOCL
            // (943+) stays outside that gate. Do not synthesize a generic muzzle
            // for a blank FX reference: the parsed name is authoritative.
            let source_has_drawable = self
                .objects
                .get(&fire.shooter_id)
                .is_some_and(|o| o.has_drawable);
            if source_has_drawable
                && !fire.fire_fx_name.trim().is_empty()
                && !fire.fire_fx_name.trim().eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&fire.shooter_id));
                let matrix = self
                    .objects
                    .get(&fire.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = self.spawn_receiving_weapon_fire_fx_with_effects(
                    fire.origin,
                    None,
                    self.frame,
                    fire.shooter_id,
                    None,
                    &fire.fire_fx_name,
                    "",
                    &fire.fire_ocl_name,
                    "",
                    0.0,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
            if !fire.fire_ocl_name.trim().is_empty()
                && !fire.fire_ocl_name.trim().eq_ignore_ascii_case("None")
            {
                let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                    &fire.fire_ocl_name,
                    Some(fire.shooter_id),
                    fire.source_team,
                    fire.source_veterancy,
                    fire.source_orientation,
                    fire.source_velocity,
                    fire.origin,
                    presentation_effects,
                );
            }
        }
    }

    /// Flush real projectile impacts after either host- or GameWorld-owned
    /// flight integration.  Keeping this at the combat boundary ensures an
    /// OCL-only detonation cannot become a presentation-only no-op.
    pub(crate) fn flush_projectile_impact_fx(&mut self) {
        self.flush_projectile_impact_fx_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn flush_projectile_impact_fx_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let impacts = self.combat_system.take_impact_fx();
        for impact in impacts {
            // C++ executes ProjectileDetonationFX and
            // ProjectileDetonationOCL independently.  An OCL-only weapon
            // (common poison/radiation field pattern) therefore still reaches
            // the real host object-creation path.
            if impact.detonation_fx_name.is_empty()
                && impact.detonation_ocl_name.is_empty()
                && impact.projectile_die_effects.is_empty()
            {
                continue;
            }
            // C++ Weapon.cpp:889-949 — ProjectileDetonationFX (drawable/doFXPos)
            // before ProjectileDetonationOCL. Keep independent empty-name skips.
            if !impact.detonation_fx_name.trim().is_empty()
                && !impact
                    .detonation_fx_name
                    .trim()
                    .eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&impact.shooter_id));
                let matrix = self
                    .objects
                    .get(&impact.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = self.dispatch_fx_list_at_host_position_oriented_with_effects(
                    &impact.detonation_fx_name,
                    impact.position,
                    Some(impact.position),
                    0.0,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
            if !impact.detonation_ocl_name.is_empty() {
                let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                    &impact.detonation_ocl_name,
                    Some(impact.shooter_id),
                    impact.source_team,
                    impact.source_veterancy,
                    impact.source_orientation,
                    impact.source_velocity,
                    impact.position,
                    presentation_effects,
                );
            }
            for effect in &impact.projectile_die_effects {
                match effect {
                    crate::game_logic::combat::ProjectileDieEffect::Fx {
                        name,
                        audio,
                        orient_to_object,
                    } => {
                        if !name.trim().is_empty() && !name.eq_ignore_ascii_case("None") {
                            let matrix = orient_to_object.then(|| {
                                glam::Mat4::from_translation(impact.position)
                                    * glam::Mat4::from_rotation_y(-impact.source_orientation)
                            });
                            let _ = self.dispatch_fx_list_at_host_position_oriented_with_effects(
                                name,
                                impact.position,
                                None,
                                0.0,
                                0.0,
                                matrix,
                                presentation_effects,
                            );
                        }
                        if let Some(audio) = audio.as_deref().filter(|name| !name.is_empty()) {
                            self.queue_audio_event(
                                crate::game_logic::AudioEventRequest::new(audio)
                                    .with_position(impact.position),
                            );
                        }
                    }
                    crate::game_logic::combat::ProjectileDieEffect::Ocl(ocl) => {
                        // Preserve the existing OCL creation, receiving owner,
                        // and impact origin. The bridge's source identity is
                        // still the firing object because virtual projectiles
                        // do not have a HostObjectStore birth receipt; C++
                        // CreateObjectDie instead passes the dying projectile.
                        // Track that attribution mismatch in hq-6lwbil rather
                        // than dropping the authored effect or inventing an ID.
                        let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                            ocl,
                            Some(impact.shooter_id),
                            impact.source_team,
                            impact.source_veterancy,
                            impact.source_orientation,
                            impact.source_velocity,
                            impact.position,
                            presentation_effects,
                        );
                    }
                }
            }
        }
    }

}

/// hq-2qedt.179: Scorpion gun/rocket refresh keys on the authored PRIMARY
/// (ScorpionTankGun family) rather than a template-name sniff; C++ crate /
/// WeaponSetUpgrade paths only flip WeaponSet conditions on the authored set.
pub(in crate::game_logic) fn authored_scorpion_gun(obj: &crate::game_logic::Object) -> bool {
    use crate::game_logic::host_scorpion::{SCORPION_TANK_GUN, SCORPION_TANK_GUN_PLUS_ONE};
    obj.authored_weapon_name_for_slot(0).is_some_and(|w| {
        w.eq_ignore_ascii_case(SCORPION_TANK_GUN)
            || w.eq_ignore_ascii_case(SCORPION_TANK_GUN_PLUS_ONE)
    })
}
