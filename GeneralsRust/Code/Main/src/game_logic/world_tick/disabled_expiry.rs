//! Original end-of-frame disabled expiry and its synchronous owner effects.
use super::super::*;
use crate::game_logic::object::TimedDisable;

// Relative order from C++ DisabledTypes.h for the modeled deadlines.
const TIMED_DISABLE_ORDER: [TimedDisable; 7] = [
    TimedDisable::Hacked,
    TimedDisable::Emp,
    TimedDisable::Held,
    TimedDisable::Paralyzed,
    TimedDisable::Underpowered,
    TimedDisable::Awestruck,
    TimedDisable::Brainwashed,
];

impl GameLogic {
    pub(in crate::game_logic) fn clear_owned_underpowered(&mut self, id: ObjectId) {
        self.clear_owned_timed_disable(id, TimedDisable::Underpowered, &mut Vec::new());
    }

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
        let mut clearing = Vec::new();
        for id in ids {
            if self.visual_world_epoch != epoch {
                return;
            }
            let Some(object) = self.objects.get(&id).filter(|object| object.is_disabled()) else {
                continue;
            };
            let birth = object.visual_object_generation;
            self.expire_object_disable_deadlines(id, birth, epoch, &mut clearing);
            if self.spawn_identity_matches(id, birth, epoch) {
                self.admit_host_radar_provider_with_effects(id, effects);
            }
        }
    }

    fn expire_object_disable_deadlines(
        &mut self,
        id: ObjectId,
        birth: u64,
        epoch: u64,
        clearing: &mut Vec<(ObjectId, TimedDisable)>,
    ) {
        let mut remaining = TIMED_DISABLE_ORDER.into_iter();
        loop {
            if self.visual_world_epoch != epoch {
                return;
            }
            let Some(object) = self.objects.get(&id)
                .filter(|object| object.visual_object_generation == birth)
            else {
                return;
            };
            let Some(effect) = remaining.find(|&effect| {
                object.timed_disable_expires_at(effect, self.frame)
            }) else {
                return;
            };
            self.clear_owned_timed_disable(id, effect, clearing);
        }
    }

    /// Object::clearDisabled visits the live SpawnBehavior roster before
    /// clearing its own mask. Slaves clear this type even when their own
    /// deadline is later.
    fn clear_owned_timed_disable(
        &mut self,
        id: ObjectId,
        effect: TimedDisable,
        clearing: &mut Vec<(ObjectId, TimedDisable)>,
    ) {
        let key = (id, effect);
        if clearing.contains(&key) {
            return;
        }
        let epoch = self.visual_world_epoch;
        let (birth, deadline) = {
            let Some(object) = self.objects.get_mut(&id) else {
                return;
            };
            let Some(deadline) = object.timed_disable_deadline(effect) else {
                return;
            };
            object.queue_timed_disable_reenabled_audio(effect, &mut self.health_events);
            if effect != TimedDisable::Held {
                object.pause_authored_powers_for_disable(false);
            }
            (object.visual_object_generation, deadline)
        };
        if deadline == game_engine::common::game_common::FOREVER
            && let Some(rider) = self.containment_disable_rider(id)
        {
            clearing.push(key);
            self.clear_owned_timed_disable(rider, effect, clearing);
            clearing.pop();
            if !self.spawn_identity_matches(id, birth, epoch) {
                return;
            }
        }
        let children = self
            .objects
            .get(&id)
            .filter(|object| object.is_spawns_are_the_weapons())
            .map(Object::spawn_behavior_children)
            .unwrap_or_default();
        if !children.is_empty() {
            clearing.push(key);
            for (child, child_birth) in children {
                if !self.spawn_identity_matches(id, birth, epoch) {
                    break;
                }
                let still_child = self.host_object(id).is_some_and(|parent| {
                    parent.contains_spawn_behavior_child(child, &child_birth)
                });
                if still_child && self.spawn_identity_matches(child, child_birth, epoch) {
                    self.clear_owned_timed_disable(child, effect, clearing);
                }
            }
            clearing.pop();
            if !self.spawn_identity_matches(id, birth, epoch) {
                return;
            }
        }
        let mask_became_clear = {
            let object = self.objects.get_mut(&id).unwrap();
            // A child's recovery can reenter the parent and clear it while
            // this invocation is suspended. C++ still runs the outer edge.
            object.clear_timed_disable_mask(effect) && !object.has_disabled_type()
        };
        // Object.cpp:2304–2308 checks from the start again after each clear.
        // An inner clear can dispatch an enable callback before this one.
        self.expire_object_disable_deadlines(id, birth, epoch, clearing);
        if !self.spawn_identity_matches(id, birth, epoch) {
            return;
        }
        let object = self.objects.get_mut(&id).unwrap();
        if object.has_disabled_type() {
            return;
        }
        object.queue_radar_disabled_callback(false);
        self.admit_host_radar_disabled_edges(id);
        if mask_became_clear {
            self.admit_owned_producer_energy_edge(id, false);
        } else {
            self.admit_producer_energy_without_mask_change(id, false);
        }
    }
}
