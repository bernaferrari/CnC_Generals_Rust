//! Host combat `impl GameLogic` — `ocl_and_scud`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

#[cfg(test)]
mod receiving_debris_ocl_owner_tests;
mod generic_objects;
mod nuke_cannon_overlord;
use generic_objects::OclObjectSource;
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

/// The OCL overload controls which source data affects a created object's pose.
#[derive(Clone, Copy)]
pub(in crate::game_logic) enum OclPrimary {
    Object { pre_physics_pose: Option<glam::Mat4> },
    /// A world position with an owning object and an independently supplied angle.
    Position,
    /// Deferred effects that carry the source heading with the recorded origin.
    FrozenSource,
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

fn unsupported_host_ocl_nugget_name(
    nugget: &dyn gamelogic::object_creation_list::ObjectCreationNugget,
) -> &'static str {
    let nugget = nugget.as_any();
    use gamelogic::object_creation_list::advanced_nuggets::{
        AttackNugget, DeliverPayloadNugget, FireWeaponNugget,
    };
    if nugget.is::<FireWeaponNugget>() {
        "FireWeapon"
    } else if nugget.is::<DeliverPayloadNugget>() {
        "DeliverPayload"
    } else if nugget.is::<AttackNugget>() {
        "Attack"
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
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
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
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
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
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
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
            return Err(error);
        }
        Ok(outcome
            .created
            .into_iter()
            .filter_map(|(id, (generation, epoch))| {
                self.spawn_identity_matches(id, generation, epoch).then_some(id)
            })
            .collect())
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
        self.execute_parsed_weapon_ocl_with_lifetime_and_births(
            ocl_name,
            OclPrimary::FrozenSource,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            secondary,
            0,
            interpreter,
            presentation_effects,
        )
    }

    /// ObjectCreationList::create(list, primary, NULL) on the installed object.
    pub(in crate::game_logic) fn execute_receiving_object_ocl_with_births(
        &mut self,
        ocl_name: &str,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        self.execute_object_ocl_with_pose(ocl_name, id, None, interpreter, effects)
    }

    fn execute_object_ocl_with_pose(
        &mut self,
        ocl_name: &str,
        id: ObjectId,
        pre_physics_pose: Option<glam::Mat4>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        let Some(object) = self.objects.get(&id) else {
            return OclExecutionOutcome::success(Vec::new());
        };
        self.execute_parsed_weapon_ocl_with_lifetime_and_births(
            ocl_name,
            OclPrimary::Object { pre_physics_pose },
            Some(id),
            object.team,
            object.experience.level,
            object.get_orientation(),
            object.movement.velocity,
            object.get_position(),
            None,
            0,
            interpreter,
            effects,
        )
    }

    /// ObjectCreationList::create(list, source, position, NULL, INVALID_ANGLE).
    /// Generic nuggets use zero for the omitted angle and receive no matrix.
    pub(in crate::game_logic) fn execute_receiving_positional_ocl_with_births(
        &mut self,
        ocl_name: &str,
        id: ObjectId,
        origin: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        let Some(object) = self.objects.get(&id) else {
            return OclExecutionOutcome::success(Vec::new());
        };
        self.execute_parsed_weapon_ocl_with_lifetime_and_births(
            ocl_name,
            OclPrimary::Position,
            Some(id),
            object.team,
            object.experience.level,
            0.0,
            object.movement.velocity,
            origin,
            None,
            0,
            None,
            effects,
        )
    }

    /// `ObjectCreationList::create` `lifetimeFrames`: a nonzero caller value
    /// overrides the authored nugget lifetime on an existing LifetimeUpdate
    /// only (ObjectCreationList.cpp:917-933); zero keeps the nugget rule.
    pub(in crate::game_logic) fn execute_parsed_weapon_ocl_with_lifetime_and_births(
        &mut self,
        ocl_name: &str,
        primary: OclPrimary,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        secondary: Option<(ObjectId, &crate::assets::LiveDrawPlaybackIdentity)>,
        lifetime_frames: u32,
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
        let Some(ocl) = self.object_creation_list_definition(ocl_name) else {
            if !matches!(primary, OclPrimary::Object { .. }) {
                log::warn!("receiving OCL `{ocl_name}` was not admitted for this GameLogic");
            }
            return OclExecutionOutcome::success(Vec::new());
        };

        // C++ ObjectCreationList.cpp:1524-1534 walks the nuggets in order.
        // Unsupported nuggets do not discard earlier results or later nuggets.
        // Reentrant callbacks may remove the captured source or reuse its ID.
        // Check the original birth before each subsequent nugget.
        let mut created = Vec::new();
        for nugget in ocl.nuggets() {
            if let Some(birth) = source_birth {
                if !source_id.is_some_and(|id| self.spawn_identity_matches(id, birth.0, birth.1)) {
                    break;
                }
            }
            if let Some(force) = nugget.as_any().downcast_ref::<
                gamelogic::object_creation_list::advanced_nuggets::ApplyRandomForceNugget,
            >() {
                if matches!(primary, OclPrimary::Object { .. })
                    && let Some(object) = source_id.and_then(|id| self.objects.get_mut(&id))
                    && object.authors_behavior_module("PhysicsBehavior")
                {
                    let applied = force.draw_force(|lo, hi| self.logic_random.next_real(lo, hi));
                    object.apply_physics_force(
                        Vec3::new(applied.x, applied.z, applied.y),
                        self.frame,
                    );
                    let [yaw, roll, pitch] =
                        force.draw_spin(|lo, hi| self.logic_random.next_real(lo, hi));
                    object.shock_yaw_rate = yaw;
                    object.shock_roll_rate = roll;
                    object.shock_pitch_rate = pitch;
                }
                continue;
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
                    if let Err(error) = self.fire_owned_temporary_weapon_at(source, weapon, position, presentation_effects) {
                        return OclExecutionOutcome::failure(created, error);
                    }
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
            if !generic.name_are_objects && generic.debris_to_generate == 0 {
                continue;
            }
            // reallyCreate checks the current controller for each nugget,
            // before reserving a container or drawing from the logic RNG.
            let source_owner_player_id = source_id
                .and_then(|id| self.objects.get(&id))
                .and_then(|source| source.owner_player_id);
            if generic.requires_live_player
                && !source_owner_player_id
                    .and_then(|owner| self.players.get(&owner))
                    .is_some_and(Player::is_active)
            {
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
            let source_transform = if let OclPrimary::Object { pre_physics_pose } = primary {
                pre_physics_pose.or_else(|| {
                    source_id.and_then(|id| self.objects.get(&id)).map(Object::get_transform_matrix)
                })
            } else {
                None
            };
            // The object overload checks altitude from raw terrain. The
            // explicit position overload has no source-altitude gate.
            if generic.skip_if_significantly_airborne
                && !matches!(primary, OclPrimary::Position)
            {
                let above = if let Some(transform) = source_transform {
                    source_id
                        .and_then(|id| self.objects.get(&id))
                        .is_some_and(|object| {
                            let position = transform.w_axis.truncate();
                            let ground = self.terrain_ground_height_at(position)
                                .unwrap_or(object.ground_height);
                            object.height_treats_as_airborne(position.y - ground)
                        })
                } else {
                    let terrain_y = self.terrain_height_at(origin).unwrap_or(origin.y);
                    crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                        origin.y - terrain_y,
                    )
                };
                if above {
                    continue;
                }
            }
            if !generic.name_are_objects {
                let debris = self.execute_receiving_create_debris(
                    generic,
                    source_id.expect("validated above"),
                    source_birth.expect("source birth captured"),
                    source_team,
                    origin,
                    matches!(primary, OclPrimary::Position).then_some(source_orientation),
                    source_transform,
                    lifetime_frames,
                    presentation_effects,
                );
                created.extend(debris.created);
                if let Some(error) = debris.error {
                    return OclExecutionOutcome::failure(created, error);
                }
                continue;
            }
            let Some((id, birth)) = source_id.zip(source_birth) else {
                return OclExecutionOutcome::success(created);
            };
            let source = OclObjectSource {
                id,
                birth,
                owner: source_owner_player_id,
                team: source_team,
                veterancy: source_veterancy,
                orientation: source_orientation,
                velocity: source_velocity,
                origin,
                primary,
                transform: source_transform,
            };
            if let Err(error) = self.create_receiving_ocl_objects(
                ocl_name,
                generic,
                source,
                lifetime_frames,
                interpreter,
                presentation_effects,
                &mut created,
            ) {
                return OclExecutionOutcome::failure(created, error);
            }
        }
        OclExecutionOutcome::success(created)
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
        positional_orientation: Option<f32>,
        source_transform: Option<glam::Mat4>,
        lifetime_frames: u32,
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
        let Some((owner_player_id, source_layer, source_forward, source_orientation)) =
            self.objects.get(&source_id).map(|source| {
                (
                    source.owner_player_id,
                    source.pathfind_layer,
                    source.thing().get_direction_vector(),
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
        let orientation = positional_orientation.unwrap_or(source_orientation);
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
            // C++ ObjectCreationList.cpp:953-960 `adjustVector(&offset, mtx)`
            // rotates the authored Z-up Offset (x forward, y left, z up) through
            // the primary Object's matrix, then adds it to the origin. The host
            // is Y-up, so the offset is applied in the object's model frame the
            // same way every other authored model-space offset is (INI X =
            // forward, INI Y = left, INI Z = up); feeding (x, y, z) straight
            // into the host matrix would rotate "y" as if it were height.
            let local_offset = Vec3::new(generic.offset.x, generic.offset.z, generic.offset.y);
            let position = if let Some(matrix) = source_transform {
                matrix.transform_point3(local_offset)
            } else if positional_orientation.is_some() {
                origin + local_offset
            } else {
                crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                    origin,
                    source_forward,
                    (generic.offset.x, generic.offset.y, generic.offset.z),
                )
            };
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
                // C++ doStuffToObj: a nonzero caller lifetime overrides the
                // nugget lifetime on the first existing LifetimeUpdate; the
                // nugget maximum applies only when no override was passed.
                // The setter consumes this receiving world's RNG after
                // factory creation.
                if lifetime_frames != 0 {
                    let _ = object.set_native_lifetime_range(
                        self.frame,
                        crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                            minimum: lifetime_frames,
                            maximum: lifetime_frames,
                        },
                        &self.logic_random,
                    );
                } else if generic.max_frames > 0 {
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
                let (force, [yaw, roll, pitch]) =
                    generic.draw_flight(|lo, hi| self.logic_random.next_real(lo, hi));
                object.apply_physics_force(Vec3::new(force.x, force.z, force.y), self.frame);
                object.set_orientation_with_partition_owner(
                    orientation,
                    &mut self.partition_manager,
                );
                object.shock_yaw_rate = yaw;
                object.shock_roll_rate = roll;
                object.shock_pitch_rate = pitch;
            }
            self.ocl_create_debris_reg
                .record_spawn(host_ocl_disposition_bits(generic.disposition));
        }
        OclExecutionOutcome::success(created)
    }

    /// Materialize FireFX and FireOCL events stamped when pending projectiles
    /// were drained. This is kept in GameLogic so both the normal host tick
    /// and GameWorld fire-spawn apply path use the same real object lifecycle.
    pub(crate) fn execute_pending_weapon_fire_ocls(&mut self) -> gamelogic::GameLogicResult<()> {
        self.execute_pending_weapon_fire_ocls_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_pending_weapon_fire_ocls_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
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
                self.execute_parsed_weapon_ocl_at_with_effects(
                    &fire.fire_ocl_name,
                    Some(fire.shooter_id),
                    fire.source_team,
                    fire.source_veterancy,
                    fire.source_orientation,
                    fire.source_velocity,
                    fire.origin,
                    presentation_effects,
                )?;
            }
        }
        Ok(())
    }

    /// Flush real projectile impacts after either host- or GameWorld-owned
    /// flight integration.  Keeping this at the combat boundary ensures an
    /// OCL-only detonation cannot become a presentation-only no-op.
    pub(crate) fn flush_projectile_impact_fx(&mut self) -> gamelogic::GameLogicResult<()> {
        self.flush_projectile_impact_fx_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn flush_projectile_impact_fx_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::ProjectileDetonationSource;
        use crate::game_logic::weapon_bootstrap::game_logic_veterancy;

        let impacts = self.combat_system.take_impact_fx();
        for impact in impacts {
            let ranked_weapon = match &impact.detonation_source {
                ProjectileDetonationSource::Object { id, generation, epoch, weapon_key, .. } => {
                    if !self.spawn_identity_matches(*id, *generation, *epoch) {
                        continue;
                    }
                    self.weapon_definitions.as_ref()
                        .and_then(|definitions| definitions.get(weapon_key))
                        .map(|rule| (rule.clone(), self.objects[id].experience.level))
                }
                _ => None,
            };
            // C++ Weapon.cpp:889-949 — ProjectileDetonationFX (drawable/doFXPos)
            // before ProjectileDetonationOCL. A named weapon's empty slot
            // is authoritative, including a rank-specific None.
            let fx_name = ranked_weapon.as_ref().map_or(impact.detonation_fx_name.as_str(),
                |(rule, rank)| rule.get_projectile_detonate_fx(game_logic_veterancy(*rank))
                    .map_or("", |fx| fx.name()),
            ).trim();
            if !fx_name.is_empty() && !fx_name.eq_ignore_ascii_case("None") {
                self.dispatch_projectile_detonation_fx(
                    &impact,
                    fx_name,
                    ranked_weapon.as_ref().map(|(rule, _)| rule.as_ref()),
                    presentation_effects,
                );
            }
            let error = match &impact.detonation_source {
                ProjectileDetonationSource::Object { id, generation, epoch, pre_physics_pose, .. } => {
                    // Weapon.cpp:943–949 invokes the Object overload on
                    // the actual projectile and rereads rank after FX.
                    if self.spawn_identity_matches(*id, *generation, *epoch) {
                        let ocl_name = ranked_weapon.as_ref().map_or(impact.detonation_ocl_name.as_str(),
                            |(rule, _)| rule.get_projectile_detonation_ocl_name(
                                game_logic_veterancy(self.objects[id].experience.level),
                            ).unwrap_or_default(),
                        );
                        self.execute_object_ocl_with_pose(
                            ocl_name,
                            *id,
                            *pre_physics_pose,
                            None,
                            presentation_effects,
                        ).error
                    } else {
                        None
                    }
                }
                source if !impact.detonation_ocl_name.is_empty() => {
                    let outcome = self.execute_parsed_weapon_ocl_with_births(
                        &impact.detonation_ocl_name,
                        Some(impact.shooter_id),
                        impact.source_team,
                        impact.source_veterancy,
                        impact.source_orientation,
                        impact.source_velocity,
                        impact.position,
                        None,
                        presentation_effects,
                    );
                    let producer = match source {
                        ProjectileDetonationSource::VirtualProjectile(id) => Some(*id),
                        _ => None,
                    };
                    for &(id, (generation, epoch)) in &outcome.created {
                        if self.spawn_identity_matches(id, generation, epoch)
                            && let Some(object) = self.objects.get_mut(&id)
                            && object.has_native_emp_update()
                        {
                            // Virtual flights lack an Object overload.
                            // Preserve the producer used by the pulse at fade.
                            object.producer_id = producer;
                        }
                    }
                    outcome.error
                }
                _ => None,
            };
            if let Some(error) = error {
                return Err(error);
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
                        let outcome = self.execute_parsed_weapon_ocl_with_births(
                            ocl,
                            Some(impact.shooter_id),
                            impact.source_team,
                            impact.source_veterancy,
                            impact.source_orientation,
                            impact.source_velocity,
                            impact.position,
                            None,
                            presentation_effects,
                        );
                        if let Some(error) = outcome.error {
                            return Err(error);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn dispatch_projectile_detonation_fx(
        &mut self,
        impact: &crate::game_logic::combat::ProjectileImpactFx,
        name: &str,
        rule: Option<&gamelogic::weapon::WeaponTemplate>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::ProjectileDetonationSource;
        use crate::presentation_frame::{FrozenWeaponVisualFxRoute, FrozenWeaponVisualSourceGate};

        let (position, matrix, speed, radius) = match &impact.detonation_source {
            ProjectileDetonationSource::Object {
                id, pre_physics_pose, primary_damage_radius, ..
            } => {
                // The caller has validated the live projectile's birth.
                let actor = &self.objects[id];
                let gate = FrozenWeaponVisualSourceGate {
                    source_has_drawable: actor.has_drawable,
                    source_is_locally_controlled: super::weapon_visual_capture::source_is_locally_controlled(
                        actor.owner_player_id, self.local_player_id(),
                    ),
                    source_is_stealthed: actor.status.stealthed,
                    source_is_detected: actor.status.detected,
                    source_is_disguised: actor.status.disguised,
                    source_is_mine: actor.is_kind_of(KindOf::Mine),
                    weapon_plays_fx_when_stealthed: rule.is_some_and(|rule| rule.play_fx_when_stealthed),
                    selected_fx_is_present: true,
                    logic_frame: self.frame,
                    // WeaponStore::handleProjectileDetonation constructs a
                    // temporary weapon this frame (Weapon.cpp:1504,1742).
                    suspend_fx_frame: self.frame.wrapping_add(rule.map_or(0, |rule| rule.suspend_fx_delay)),
                };
                if gate.fx_route() != Some(FrozenWeaponVisualFxRoute::BroadcastWithFireFx) {
                    return;
                }
                #[cfg(feature = "game_client")]
                let drawable_pose = effects.view.as_deref_mut().and_then(|view| {
                    let source = self.current_native_slave_source(*id, view.local_player_id()).ok()?;
                    view.detonation_weapon_fx_pose(&source, *pre_physics_pose)
                });
                #[cfg(not(feature = "game_client"))]
                let drawable_pose: Option<glam::Mat4> = None;

                // Weapon.cpp:936–939, when the Drawable did not handle FX.
                // A contact weapon changes the primary point, not its basis.
                let pose = drawable_pose.or(*pre_physics_pose).unwrap_or_else(|| actor.get_transform_matrix());
                let position = if drawable_pose.is_none()
                    && rule.is_some_and(gamelogic::weapon::WeaponTemplate::is_contact_weapon)
                {
                    impact.position
                } else {
                    pose.w_axis.truncate()
                };
                (position, Some(pose), rule.map_or(0.0, |rule| rule.weapon_speed), *primary_damage_radius)
            }
            _ => {
                let shooter = self.objects.get(&impact.shooter_id);
                (
                    impact.position,
                    shooter.map(Object::get_transform_matrix),
                    0.0,
                    host_fire_fx_override_radius(shooter),
                )
            }
        };
        let _ = self.dispatch_fx_list_at_host_position_oriented_with_effects(
            name, position, Some(impact.position), speed, radius, matrix, effects,
        );
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
