//! Original end-of-frame disabled expiry and its synchronous owner effects.
use super::super::*;
use crate::game_logic::object::TimedDisable;

impl GameLogic {
    pub(in super::super) fn expire_owned_disabled_statuses(&mut self) {
        self.expire_owned_disabled_statuses_with_effects(&mut Default::default())
    }

    pub(in super::super) fn expire_owned_disabled_statuses_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ GameLogic.cpp:3783-3792 walks the current live object list after
        // destruction and victory. Release each Object borrow before admitting
        // its Player callbacks; no object is temporarily removed from its store.
        let ids: Vec<_> = self.objects.registered_keys().copied().collect();
        let epoch = self.visual_world_epoch;
        let mut ancestors = Vec::new();
        'actor: for id in ids {
            if self.visual_world_epoch != epoch {
                return;
            }
            let Some(object) = self.objects.get(&id).filter(|object| object.is_disabled()) else {
                continue;
            };
            let birth = object.visual_object_generation;
            for effect in [TimedDisable::Hacked, TimedDisable::Emp, TimedDisable::Paralyzed] {
                let due = self.objects.get(&id).is_some_and(|object| {
                    object.timed_disable_expires_at(effect, self.frame)
                });
                if due {
                    self.clear_owned_timed_disable(id, effect, &mut ancestors);
                    if self.visual_world_epoch != epoch {
                        return;
                    }
                    if !self
                        .objects
                        .get(&id)
                        .is_some_and(|object| object.visual_object_generation == birth)
                    {
                        continue 'actor;
                    }
                }
            }

            let Some(object) = self.objects.get_mut(&id) else {
                continue;
            };
            let was_disabled = object.has_disabled_type();
            object.tick_remaining_disabled(self.frame);
            let reenabled = was_disabled && !object.has_disabled_type();
            self.admit_host_radar_provider_with_effects(id, effects);
            if reenabled {
                self.admit_owned_producer_energy_edge(id, false);
            }
        }
    }

    /// Object::clearDisabled visits the live SpawnBehavior roster before
    /// clearing its own mask. Slaves clear this type even when their own
    /// deadline is later.
    fn clear_owned_timed_disable(
        &mut self,
        id: ObjectId,
        effect: TimedDisable,
        ancestors: &mut Vec<ObjectId>,
    ) {
        if ancestors.contains(&id) {
            return;
        }
        let epoch = self.visual_world_epoch;
        let (birth, children) = {
            let Some(object) = self.objects.get(&id) else {
                return;
            };
            if object.timed_disable_deadline(effect).is_none() {
                return;
            }
            object.queue_timed_disable_reenabled_audio(effect, &mut self.health_events);
            let children = if object.is_spawns_are_the_weapons() {
                object.spawn_behavior_children()
            } else {
                Vec::new()
            };
            (object.visual_object_generation, children)
        };
        if !children.is_empty() {
            ancestors.push(id);
            for (child, child_birth) in children {
                if !self.spawn_identity_matches(id, birth, epoch) {
                    break;
                }
                let still_child = self.host_object(id).is_some_and(|parent| {
                    parent.contains_spawn_behavior_child(child, &child_birth)
                });
                if still_child && self.spawn_identity_matches(child, child_birth, epoch) {
                    self.clear_owned_timed_disable(child, effect, ancestors);
                }
            }
            ancestors.pop();
            if !self.spawn_identity_matches(id, birth, epoch) {
                return;
            }
        }
        let reenabled = {
            let object = self.objects.get_mut(&id).unwrap();
            if !object.clear_timed_disable(effect) {
                return;
            }
            !object.has_disabled_type()
        };
        self.admit_host_radar_disabled_edges(id);
        if reenabled {
            self.admit_owned_producer_energy_edge(id, false);
        }
    }
}
