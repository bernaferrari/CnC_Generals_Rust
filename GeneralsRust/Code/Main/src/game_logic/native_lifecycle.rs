//! Object-owned authored update modules. Each slot retains its own continuation;
//! immediate engine effects run after ending the bounded Object borrow.
use super::native_lifecycle_definition::{
    HeightDieDefinition, LifetimeFrameRange, NativeLifecycleDefinition, NativeLifecycleKind,
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
            NativeLifecycleState::HeightDie { .. } => return None,
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
                .all(|(definition, state)| {
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
                            _ => false,
                        }
                })
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
    pub(crate) fn bind_native_lifecycle(
        &mut self,
        frame: u32,
        random: &game_engine::common::random_value::SessionRandom,
        hulk_override: i32,
    ) {
        let Some(definitions) = self.thing().template.native_lifecycle_definitions.as_ref() else {
            return;
        };
        let is_hulk = self.is_kind_of(KindOf::Hulk);
        let states = definitions
            .iter()
            .map(|definition| match definition.kind {
                NativeLifecycleKind::Lifetime(mut range) => {
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
                    die_frame: deadline(frame, range, random),
                    active: true,
                },
                NativeLifecycleKind::HeightDie(_) => NativeLifecycleState::HeightDie {
                    earliest_frame: u32::MAX,
                    last_position: Vec3::splat(-1.0),
                    has_died: false,
                    particles_destroyed: false,
                },
            })
            .collect();
        self.native_lifecycle = Some(NativeLifecycleRuntime {
            definitions: definitions.clone(),
            states,
        });
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
    pub(crate) fn tick_native_lifecycle(&mut self, id: ObjectId) {
        self.tick_native_lifecycle_with_effects(id, &mut Default::default())
    }

    pub(crate) fn tick_native_lifecycle_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.drive_native_lifecycle_with_effects(id, false, effects);
    }
    /// Field expiry has its existing phase, but must not advance HeightDie twice.
    pub(crate) fn tick_native_lifetime_timers(&mut self, id: ObjectId) {
        self.tick_native_lifetime_timers_with_effects(id, &mut Default::default())
    }

    pub(crate) fn tick_native_lifetime_timers_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.drive_native_lifecycle_with_effects(id, true, effects);
    }
    fn drive_native_lifecycle(&mut self, id: ObjectId, timers_only: bool) {
        self.drive_native_lifecycle_with_effects(id, timers_only, &mut Default::default())
    }

    fn drive_native_lifecycle_with_effects(
        &mut self,
        id: ObjectId,
        timers_only: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        let Some(runtime) = object.native_lifecycle.as_ref() else {
            return;
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
            // All three original modules inherit DISABLEDMASK_NONE. The
            // scheduler retries next frame without consuming their continuation.
            if object.has_disabled_type() {
                return;
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
                        self.destroy_object_with_effects(id, effects);
                    } else {
                        self.apply_owned_kill_with_effects(
                            id,
                            super::combat::DamageType::Unresistable,
                            super::host_usa_pilot::HostDeathType::Normal,
                            effects,
                        );
                    }
                }
                NativeLifecycleKind::HeightDie(definition) => {
                    if !timers_only {
                        self.tick_native_height_die_with_effects(
                            id, index, &identity, definition, effects,
                        );
                    }
                }
            }
        }
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
    ) {
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
    ) {
        let pose_frame = self.frame;
        let Some(object) = self.objects.get_mut(&id) else {
            return;
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
            return;
        };
        if *earliest_frame == u32::MAX {
            *earliest_frame = self.frame.wrapping_add(definition.initial_delay);
        }
        if self.frame < *earliest_frame {
            return;
        }
        if contained {
            *last_position = position;
            return;
        }
        let direction_ok = !definition.only_when_moving_down || position.y < last_position.y;
        let may_die = !*has_died && direction_ok;
        let Some((terrain, target)) = self.native_height_die_surface(id, definition) else {
            return;
        };
        if may_die && position.y < target {
            if definition.snap_to_ground_on_death || position.y < terrain {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.set_position_with_effects(
                        Vec3::new(position.x, terrain, position.z),
                        pose_frame,
                        &mut effects.reborrow_for_object(&mut self.partition_manager),
                    );
                }
            }
            self.apply_owned_kill_with_effects(
                id,
                super::combat::DamageType::Unresistable,
                super::host_usa_pilot::HostDeathType::Normal,
                effects,
            );
            let Some(object) = self.objects.get_mut(&id) else {
                return;
            };
            if !std::sync::Arc::ptr_eq(identity, &object.visual_draw_identity_token) {
                return;
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
            return;
        };
        if !std::sync::Arc::ptr_eq(identity, &object.visual_draw_identity_token) {
            return;
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
            return;
        };
        let destroy_particles = !*particles_destroyed
            && current.y < definition.destroy_attached_particles_at_height
            && (*has_died || direction_ok);
        if destroy_particles {
            *particles_destroyed = true;
        }
        *last_position = current;
        if destroy_particles {
            self.combat_particles_mut().destroy_attached_to_object(id);
        }
    }
}
