//! Object-owned authored update modules. Each slot retains its own continuation;
//! immediate engine effects run after ending the bounded Object borrow.
use super::native_lifecycle_definition::{
    EmpUpdateDefinition, FireOclAfterCooldownDefinition, HeightDieDefinition, LifetimeFrameRange,
    NativeLifecycleDefinition, NativeLifecycleKind,
};
use super::{GameLogic, KindOf, Object, ObjectId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum NativeLifecycleState {
    Lifetime {
        die_frame: u32,
        active: bool,
    },
    Deletion {
        die_frame: u32,
        active: bool,
    },
    HeightDie {
        earliest_frame: u32,
        last_position: Vec3,
        has_died: bool,
        particles_destroyed: bool,
    },
    FireWeapon {
        initial_delay_frame: u32,
        weapon: Option<crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponRuntimeState>,
    },
    Emp(EmpUpdateState),
    /// C++ FireOCLAfterWeaponCooldownUpdate `m_valid` + resetStats fields.
    FireOclAfterCooldown {
        valid: bool,
        consecutive_shots: u32,
        start_frame: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EmpUpdateState {
    pub(crate) active: bool,
    pub(crate) fade_frame: u32,
    pub(crate) die_frame: u32,
    pub(crate) current_scale: f32,
    pub(crate) target_scale: f32,
}

impl EmpUpdateState {
    pub(crate) fn import_inactive(definition: &EmpUpdateDefinition) -> Self {
        Self {
            active: false,
            fade_frame: 0,
            die_frame: 0,
            current_scale: definition.start_scale,
            target_scale: definition.start_scale,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct NativeLifecycleRuntime {
    pub(crate) definitions: Box<[NativeLifecycleDefinition]>,
    pub(crate) states: Box<[NativeLifecycleState]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeTimerAction {
    Kill,
    Delete,
}

impl NativeLifecycleRuntime {
    /// Consume one scheduled wake while borrowed; its immediate effect is
    /// executed by the receiving world after this bounded phase.
    pub(crate) fn take_due_timer(&mut self, index: usize, frame: u32) -> Option<NativeTimerAction> {
        let (die_frame, active, action) = match self.states.get_mut(index)? {
            NativeLifecycleState::Lifetime { die_frame, active } => {
                (die_frame, active, NativeTimerAction::Kill)
            }
            NativeLifecycleState::Deletion { die_frame, active } => {
                (die_frame, active, NativeTimerAction::Delete)
            }
            NativeLifecycleState::HeightDie { .. }
            | NativeLifecycleState::FireWeapon { .. }
            | NativeLifecycleState::Emp(_)
            | NativeLifecycleState::FireOclAfterCooldown { .. } => return None,
        };
        if !*active || frame < *die_frame {
            return None;
        }
        *active = false;
        Some(action)
    }

    pub(crate) fn validate(&self) -> bool {
        let mut tags = std::collections::HashSet::new();
        if self
            .definitions
            .iter()
            .filter_map(|definition| definition.module_tag.as_ref())
            .any(|tag| !tags.insert(tag))
        {
            return false;
        }
        self.definitions.len() == self.states.len()
            && self
                .definitions
                .iter()
                .zip(self.states.iter())
                .enumerate()
                .all(|(index, (definition, state))| {
                    definition.validate().is_ok()
                        && match (&definition.kind, state) {
                            (
                                NativeLifecycleKind::Lifetime(_),
                                NativeLifecycleState::Lifetime { .. },
                            )
                            | (
                                NativeLifecycleKind::Deletion(_),
                                NativeLifecycleState::Deletion { .. },
                            ) => true,
                            (
                                NativeLifecycleKind::HeightDie(_),
                                NativeLifecycleState::HeightDie { last_position, .. },
                            ) => last_position.is_finite(),
                            (NativeLifecycleKind::FireWeapon(definition), NativeLifecycleState::FireWeapon { weapon, .. }) => {
                                weapon.as_ref().is_none_or(|weapon| {
                                    weapon.key.module_source_index as usize == index
                                        && weapon.weapon_template_name.eq_ignore_ascii_case(&definition.weapon_name)
                                })
                            }
                            (NativeLifecycleKind::Emp(definition), NativeLifecycleState::Emp(state)) => {
                                state.current_scale.is_finite()
                                    && state.target_scale.is_finite()
                                    && (!state.active
                                        || state.die_frame.wrapping_sub(state.fade_frame)
                                            == definition.lifetime_frames - definition.start_fade_frames)
                            }
                            (
                                NativeLifecycleKind::FireOclAfterCooldown(_),
                                NativeLifecycleState::FireOclAfterCooldown { .. },
                            ) => true,
                            _ => false,
                        }
                })
    }
    pub(crate) fn has_emp_update(&self) -> bool {
        self.states
            .iter()
            .any(|state| matches!(state, NativeLifecycleState::Emp(_)))
    }

    pub(crate) fn first_lifetime_deadline(&self) -> Option<u32> {
        self.states.iter().find_map(|state| match state {
            NativeLifecycleState::Lifetime { die_frame, .. } => Some(*die_frame),
            _ => None,
        })
    }
}
fn deadline(
    frame: u32,
    range: LifetimeFrameRange,
    random: &game_engine::common::random_value::SessionRandom,
) -> u32 {
    let width = u64::from(range.maximum) - u64::from(range.minimum) + 1;
    let drawn = u64::from(range.minimum) + u64::from(random.next_u32()) % width;
    frame.wrapping_add((drawn as u32).max(1))
}
impl Object {
    pub(crate) fn has_native_emp_update(&self) -> bool {
        self.native_lifecycle
            .as_ref()
            .is_some_and(NativeLifecycleRuntime::has_emp_update)
    }

    pub(crate) fn bind_native_lifecycle(
        &mut self,
        frame: u32,
        random: &game_engine::common::random_value::SessionRandom,
        hulk_override: i32,
        weapon_definitions: Option<&generals_sim::weapon::WeaponCatalog>,
    ) {
        let Some(definitions) = self.thing().template.native_lifecycle_definitions.as_ref() else {
            return;
        };
        let is_hulk = self.is_kind_of(KindOf::Hulk);
        let mut emp_initial_pose = None;
        let states = definitions
            .iter()
            .enumerate()
            .map(|(index, definition)| match &definition.kind {
                NativeLifecycleKind::Lifetime(range) => {
                    let mut range = *range;
                    if is_hulk && hulk_override != -1 {
                        range = LifetimeFrameRange {
                            minimum: hulk_override.max(0) as u32,
                            maximum: hulk_override.max(0) as u32,
                        };
                    }
                    NativeLifecycleState::Lifetime {
                        die_frame: deadline(frame, range, random),
                        active: true,
                    }
                }
                NativeLifecycleKind::Deletion(range) => NativeLifecycleState::Deletion {
                    die_frame: deadline(frame, *range, random),
                    active: true,
                },
                NativeLifecycleKind::HeightDie(_) => NativeLifecycleState::HeightDie {
                    earliest_frame: u32::MAX,
                    last_position: Vec3::splat(-1.0),
                    has_died: false,
                    particles_destroyed: false,
                },
                NativeLifecycleKind::FireWeapon(definition) => {
                    use crate::game_logic::host_temporary_weapon_behavior::{
                        FireWeaponWhenDamagedWeaponRole, TemporaryWeaponConstructionDefaults,
                        TemporaryWeaponRuntimeKey, TemporaryWeaponRuntimeSpec,
                        TemporaryWeaponRuntimeState, TemporaryWeaponSlot,
                    };
                    let weapon = weapon_definitions
                        .and_then(|definitions| definitions.get(&definition.weapon_name.to_ascii_lowercase()))
                        .map(|template| {
                            let spec = TemporaryWeaponRuntimeSpec {
                                key: TemporaryWeaponRuntimeKey {
                                    module_source_index: index as u32,
                                    role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
                                },
                                weapon_template_name: definition.weapon_name.clone(),
                                weapon_slot: TemporaryWeaponSlot::Primary,
                            };
                            let mut defaults = TemporaryWeaponConstructionDefaults::from_weapon_template(template);
                            let mut weapon = TemporaryWeaponRuntimeState::from_cxx_constructor(&spec, defaults, frame);
                            defaults.clip_reload_frames = 0;
                            weapon.reload_ammo_from_cxx(defaults, frame);
                            weapon
                        });
                    NativeLifecycleState::FireWeapon {
                        initial_delay_frame: frame.wrapping_add(definition.initial_delay),
                        weapon,
                    }
                }
                // FireOCLAfterWeaponCooldownUpdate.cpp:68-73 — constructor binds
                // m_valid=false and resetStats; no clock or RNG draw.
                NativeLifecycleKind::FireOclAfterCooldown(_) => {
                    NativeLifecycleState::FireOclAfterCooldown {
                        valid: false,
                        consecutive_shots: 0,
                        start_frame: 0,
                    }
                }
                NativeLifecycleKind::Emp(definition) => {
                    // EMPUpdate.cpp:74–86 draws the target scale before orientation.
                    let target_scale = random.next_real(
                        definition.target_scale_min,
                        definition.target_scale_max,
                    );
                    let yaw = random.next_real(-std::f32::consts::PI, std::f32::consts::PI);
                    emp_initial_pose = Some((definition.start_scale, yaw));
                    NativeLifecycleState::Emp(EmpUpdateState {
                        active: true,
                        fade_frame: frame.wrapping_add(definition.start_fade_frames),
                        die_frame: frame.wrapping_add(definition.lifetime_frames),
                        current_scale: definition.start_scale,
                        target_scale,
                    })
                }
            })
            .collect();
        self.native_lifecycle = Some(NativeLifecycleRuntime {
            definitions: definitions.clone(),
            states,
        });
        if let Some((scale, yaw)) = emp_initial_pose {
            self.drawable_instance_scale = scale;
            self.set_orientation(yaw);
        }
    }
    /// OCL finds the first existing LifetimeUpdate; it never creates a module.
    pub(crate) fn set_native_lifetime_range(
        &mut self,
        frame: u32,
        range: LifetimeFrameRange,
        random: &game_engine::common::random_value::SessionRandom,
    ) -> bool {
        let Some(runtime) = self.native_lifecycle.as_mut() else {
            return false;
        };
        let Some(state) = runtime
            .states
            .iter_mut()
            .find(|state| matches!(state, NativeLifecycleState::Lifetime { .. }))
        else {
            return false;
        };
        *state = NativeLifecycleState::Lifetime {
            die_frame: deadline(frame, range, random),
            active: true,
        };
        true
    }
}
impl GameLogic {
    pub(crate) fn tick_native_lifecycle(&mut self, id: ObjectId) -> generals_sim::GameLogicResult<()> {
        self.tick_native_lifecycle_with_effects(id, &mut Default::default())
    }

    pub(crate) fn tick_native_lifecycle_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.drive_native_lifecycle_with_effects(id, false, effects)
    }
    /// Field expiry has its existing phase, but must not advance HeightDie twice.
    pub(crate) fn tick_native_lifetime_timers(&mut self, id: ObjectId) -> generals_sim::GameLogicResult<()> {
        self.tick_native_lifetime_timers_with_effects(id, &mut Default::default())
    }

    pub(crate) fn tick_native_lifetime_timers_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.drive_native_lifecycle_with_effects(id, true, effects)
    }
    fn drive_native_lifecycle(&mut self, id: ObjectId, timers_only: bool) -> generals_sim::GameLogicResult<()> {
        self.drive_native_lifecycle_with_effects(id, timers_only, &mut Default::default())
    }

    fn drive_native_lifecycle_with_effects(
        &mut self,
        id: ObjectId,
        timers_only: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let Some(runtime) = object.native_lifecycle.as_ref() else {
            return Ok(());
        };
        let count = runtime.states.len();
        // Existing object incarnation token guards synchronous callback replacement.
        let identity = object.visual_draw_identity_token.clone();
        for index in 0..count {
            let Some(object) = self.objects.get(&id) else {
                break;
            };
            if !std::sync::Arc::ptr_eq(&identity, &object.visual_draw_identity_token) {
                break;
            }
            // The original lifecycle modules inherit DISABLEDMASK_NONE. The
            // scheduler retries next frame without consuming their continuation.
            if object.has_disabled_type() {
                return Ok(());
            }
            let Some(kind) = object
                .native_lifecycle
                .as_ref()
                .and_then(|runtime| runtime.definitions.get(index))
                .map(|definition| definition.kind.clone())
            else {
                break;
            };
            match kind {
                NativeLifecycleKind::Lifetime(_) | NativeLifecycleKind::Deletion(_) => {
                    let action = self
                        .objects
                        .get_mut(&id)
                        .and_then(|object| object.native_lifecycle.as_mut())
                        .and_then(|runtime| runtime.take_due_timer(index, self.frame));
                    let Some(action) = action else {
                        continue;
                    };
                    if action == NativeTimerAction::Delete {
                        self.destroy_object_with_effects(id, effects)?;
                    } else {
                        self.apply_owned_kill_with_effects(
                            id,
                            super::combat::DamageType::Unresistable,
                            super::host_usa_pilot::HostDeathType::Normal,
                            effects,
                        )?;
                    }
                }
                NativeLifecycleKind::FireWeapon(_) => {
                    if !timers_only {
                        self.tick_native_fire_weapon_update_with_effects(id, index, effects)?;
                    }
                }
                NativeLifecycleKind::HeightDie(definition) => {
                    if !timers_only {
                        self.tick_native_height_die_with_effects(
                            id, index, &identity, definition, effects,
                        )?;
                    }
                }
                NativeLifecycleKind::Emp(definition) => {
                    if !timers_only {
                        self.tick_native_emp_with_effects(id, index, &identity, &definition, effects)?;
                    }
                }
                NativeLifecycleKind::FireOclAfterCooldown(definition) => {
                    if !timers_only {
                        self.tick_native_fire_ocl_after_cooldown(id, index, &definition, effects)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// C++ FireOCLAfterWeaponCooldownUpdate::update (lines 78–146) and fireOCL
    /// (lines 156–172): count consecutive shots on the configured slot, and
    /// create the authored OCL when the weapon could have fired but did not.
    fn tick_native_fire_ocl_after_cooldown(
        &mut self,
        id: ObjectId,
        index: usize,
        def: &FireOclAfterCooldownDefinition,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let frame = self.frame;
        let (
            last_shot,
            next_shot,
            selected,
            can_fire,
            team,
            veterancy,
            orientation,
            velocity,
            position,
            identity,
        ) = {
            let Some(object) = self.objects.get(&id) else {
                return Ok(());
            };
            let weapon = object.weapon_slot(def.slot);
            let selected = object.selected_weapon_slot() == Some(def.slot);
            // C++ maskToCheck = controlling player's completed upgrades plus
            // the object's completed upgrades; name-keyed masks are
            // case-insensitive.
            let player = object
                .owner_player_id
                .and_then(|pid| self.get_player(pid));
            let has = |name: &str| {
                object
                    .applied_upgrades
                    .iter()
                    .any(|owned| owned.eq_ignore_ascii_case(name))
                    || player.is_some_and(|player| {
                        player
                            .completed_upgrades
                            .iter()
                            .any(|owned| owned.eq_ignore_ascii_case(name))
                    })
            };
            // UpgradeModule.cpp:150-186 — empty triggers pass absent conflicts.
            let can_fire = !def.upgrades.conflicts_with.iter().any(|name| has(name))
                && (def.upgrades.triggered_by.is_empty()
                    || def.upgrades.satisfied_by(&has));
            (
                weapon.map(|weapon| weapon.last_fire_frame),
                weapon.map(|weapon| weapon.next_fire_frame),
                selected,
                can_fire,
                object.team,
                object.experience.level,
                object.get_orientation(),
                object.movement.velocity,
                object.get_position(),
                self.live_draw_playback_identity(object),
            )
        };
        let valid_now = selected && can_fire;
        let lifetime = {
            let Some(object) = self.objects.get_mut(&id) else {
                return Ok(());
            };
            let Some(runtime) = object.native_lifecycle.as_mut() else {
                return Ok(());
            };
            let Some(NativeLifecycleState::FireOclAfterCooldown {
                valid,
                consecutive_shots,
                start_frame,
            }) = runtime.states.get_mut(index)
            else {
                return Ok(());
            };
            // A weapon that still cannot fire (no weapon or the committed
            // deadline is not yet past) only accumulates shots.
            let fire = if valid_now {
                if last_shot == Some(frame.wrapping_sub(1)) {
                    *consecutive_shots = consecutive_shots.wrapping_add(1);
                    if *consecutive_shots == 1 {
                        *start_frame = frame;
                    }
                    false
                } else {
                    next_shot.is_some_and(|next| next < frame)
                        && *consecutive_shots >= def.min_shots
                }
            } else {
                !selected && last_shot.is_some() && *consecutive_shots >= def.min_shots
            };
            // C++ fireOCL: lifetime scales with the firing streak duration.
            let lifetime = fire.then(|| {
                let seconds = frame.wrapping_sub(*start_frame) as f32 * (1.0_f32 / 30.0_f32);
                let seconds = seconds * (def.lifetime_per_second as f32 * 0.001_f32);
                ((seconds * 30.0_f32) as u32).min(def.max_frames)
            });
            if fire || *valid != valid_now {
                *consecutive_shots = 0;
                *start_frame = 0;
            }
            *valid = valid_now;
            lifetime
        };
        let (Some(lifetime), Some(ocl)) = (lifetime, def.ocl.as_deref()) else {
            return Ok(());
        };
        // ObjectCreationList::create(m_ocl, obj, obj, oclFrames).
        let outcome = self.execute_parsed_weapon_ocl_with_lifetime_and_births(
            ocl,
            super::game_logic::OclPrimary::Object { pre_physics_pose: None },
            Some(id),
            team,
            veterancy,
            orientation,
            velocity,
            position,
            Some((id, &identity)),
            lifetime,
            None,
            effects,
        );
        outcome.error.map_or(Ok(()), Err)
    }

    fn native_height_die_surface(
        &self,
        id: ObjectId,
        definition: HeightDieDefinition,
    ) -> Option<(f32, f32)> {
        let object = self.objects.get(&id)?;
        let position = object.get_position();
        let ground = self
            .terrain_ground_height_at(position)
            .unwrap_or(object.ground_height);
        let terrain = if definition.target_height_includes_structures {
            self.terrain_height_at(position)
                .unwrap_or(ground)
                .max(ground)
        } else {
            ground
        };
        let mut extra = definition.target_height;
        if definition.target_height_includes_structures {
            let range = (*object.geometry_info()).bounding_circle_radius();
            let owner_sphere = (*object.geometry_info()).bounding_sphere_radius();
            let mut tallest = 0.0f32;
            for (other_id, other) in self.objects.registered() {
                if *other_id == id || !other.is_kind_of(KindOf::Structure) {
                    continue;
                }
                let geometry = &(*other.geometry_info());
                let center = |position: Vec3, geometry: &super::thing::HostGeometryInfo| {
                    let offset = if geometry.geom_type == super::thing::HostGeometryType::Sphere {
                        0.0
                    } else {
                        geometry.height * 0.5
                    };
                    position + Vec3::Y * offset
                };
                let distance = (center(position, &(*object.geometry_info()))
                    .distance(center(other.get_position(), geometry))
                    - owner_sphere
                    - geometry.bounding_sphere_radius())
                .max(0.0);
                if distance * distance < range * range {
                    tallest = tallest.max(geometry.max_height_above_position());
                }
            }
            extra = extra.max(tallest);
        }
        Some((terrain, terrain + extra))
    }
    fn tick_native_height_die(
        &mut self,
        id: ObjectId,
        index: usize,
        identity: &std::sync::Arc<()>,
        definition: HeightDieDefinition,
    ) -> generals_sim::GameLogicResult<()> {
        self.tick_native_height_die_with_effects(
            id,
            index,
            identity,
            definition,
            &mut Default::default(),
        )
    }

    fn tick_native_height_die_with_effects(
        &mut self,
        id: ObjectId,
        index: usize,
        identity: &std::sync::Arc<()>,
        definition: HeightDieDefinition,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let pose_frame = self.frame;
        let Some(object) = self.objects.get_mut(&id) else {
            return Ok(());
        };
        let position = object.get_position();
        let contained = object.contained_by.is_some();
        let Some(NativeLifecycleState::HeightDie {
            earliest_frame,
            last_position,
            has_died,
            ..
        }) = object
            .native_lifecycle
            .as_mut()
            .and_then(|runtime| runtime.states.get_mut(index))
        else {
            return Ok(());
        };
        if *earliest_frame == u32::MAX {
            *earliest_frame = self.frame.wrapping_add(definition.initial_delay);
        }
        if self.frame < *earliest_frame {
            return Ok(());
        }
        if contained {
            *last_position = position;
            return Ok(());
        }
        let direction_ok = !definition.only_when_moving_down || position.y < last_position.y;
        let may_die = !*has_died && direction_ok;
        let Some((terrain, target)) = self.native_height_die_surface(id, definition) else {
            return Ok(());
        };
        if may_die && position.y < target {
            if definition.snap_to_ground_on_death || position.y < terrain {
                if !self.set_owned_object_position_with_effects(
                    id,
                    Vec3::new(position.x, terrain, position.z),
                    pose_frame,
                    effects,
                ) {
                    return Ok(());
                }
            }
            self.apply_owned_kill_with_effects(
                id,
                super::combat::DamageType::Unresistable,
                super::host_usa_pilot::HostDeathType::Normal,
                effects,
            )?;
            let Some(object) = self.objects.get_mut(&id) else {
                return Ok(());
            };
            if !std::sync::Arc::ptr_eq(identity, &object.visual_draw_identity_token) {
                return Ok(());
            }
            if let Some(NativeLifecycleState::HeightDie { has_died, .. }) = object
                .native_lifecycle
                .as_mut()
                .and_then(|runtime| runtime.states.get_mut(index))
            {
                *has_died = true;
            }
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return Ok(());
        };
        if !std::sync::Arc::ptr_eq(identity, &object.visual_draw_identity_token) {
            return Ok(());
        }
        let current = object.get_position();
        let Some(NativeLifecycleState::HeightDie {
            last_position,
            has_died,
            particles_destroyed,
            ..
        }) = object
            .native_lifecycle
            .as_mut()
            .and_then(|runtime| runtime.states.get_mut(index))
        else {
            return Ok(());
        };
        let destroy_particles = !*particles_destroyed
            && current.y < definition.destroy_attached_particles_at_height
            && (*has_died || direction_ok);
        if destroy_particles {
            *particles_destroyed = true;
        }
        *last_position = current;
        if destroy_particles {
            self.combat_particles_mut()
                .destroy_attached_to_object(id, effects);
        }
        Ok(())
    }
}
