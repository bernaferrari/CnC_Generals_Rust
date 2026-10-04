//! Synchronous max-health change on the driving Object and its one cached body.
//! C++ ActiveBody.cpp:873-922,930-943,952-1178,1188-1227. No owner is retained.
use super::body_module::{
    BodyModuleInterface, BodyResult, MaxHealthChangeType, OwnerMaxHealthChange,
};
use crate::common::ObjectStatusMaskType;
use crate::helpers::{TheParticleSystemManager, game_client_random_value};
use crate::object::{Object, drawable::DrawableExt};
use game_engine::common::global_data;
use std::sync::{Arc, Mutex};

pub(super) struct ParticleGroup {
    pub prefix: String,
    pub system: String,
    pub count: i32,
}

/// Same authored grouping for the owner-aware path and legacy body adapter.
/// The plan owns only temporary definition strings; it never mirrors runtime.
pub(super) fn body_particle_groups(aflame: bool) -> Vec<ParticleGroup> {
    let Ok(data) = global_data::read_safe() else {
        return Vec::new();
    };
    let multiplier = if aflame { 2 } else { 1 };
    let mut groups = vec![
        (
            &data.auto_fire_particle_small_prefix,
            if aflame {
                &data.auto_fire_particle_medium_system
            } else {
                &data.auto_fire_particle_small_system
            },
            data.auto_fire_particle_small_max,
        ),
        (
            &data.auto_fire_particle_medium_prefix,
            if aflame {
                &data.auto_fire_particle_large_system
            } else {
                &data.auto_fire_particle_medium_system
            },
            data.auto_fire_particle_medium_max,
        ),
        (
            &data.auto_fire_particle_large_prefix,
            &data.auto_fire_particle_large_system,
            data.auto_fire_particle_large_max,
        ),
        (
            &data.auto_smoke_particle_small_prefix,
            if aflame {
                &data.auto_fire_particle_small_system
            } else {
                &data.auto_smoke_particle_small_system
            },
            data.auto_smoke_particle_small_max,
        ),
        (
            &data.auto_smoke_particle_medium_prefix,
            if aflame {
                &data.auto_fire_particle_small_system
            } else {
                &data.auto_smoke_particle_medium_system
            },
            data.auto_smoke_particle_medium_max,
        ),
        (
            &data.auto_smoke_particle_large_prefix,
            if aflame {
                &data.auto_fire_particle_small_system
            } else {
                &data.auto_smoke_particle_large_system
            },
            data.auto_smoke_particle_large_max,
        ),
    ];
    if aflame {
        groups.push((
            &data.auto_aflame_particle_prefix,
            &data.auto_aflame_particle_system,
            data.auto_aflame_particle_max,
        ));
    }
    groups
        .into_iter()
        .map(|(prefix, system, count)| ParticleGroup {
            prefix: prefix.clone(),
            system: system.clone(),
            count: count.saturating_mul(multiplier),
        })
        .collect()
}

type Body = Arc<Mutex<dyn BodyModuleInterface>>;

impl Object {
    /// The caller owns this Object. Each internalChangeHealth reaction finishes
    /// synchronously before the next cap check; effects never hold the body.
    pub(crate) fn add_body_max_health_with_owner(
        &mut self,
        addition: f32,
        kind: MaxHealthChangeType,
    ) -> BodyResult<()> {
        let Some(body) = self.get_body_module() else {
            return Ok(());
        };
        let (max_health, operation) = {
            let mut body = body.lock().expect("max health body poisoned");
            let max_health = body.get_max_health() + addition;
            let operation = body.begin_owner_max_health_change(max_health, kind)?;
            (max_health, operation)
        };
        let OwnerMaxHealthChange::Active { first_delta } = operation else {
            return Ok(());
        };
        if let Some(delta) = first_delta {
            self.change_body_health_with_owner(&body, delta)?;
        }
        let now = body.lock().expect("max health body poisoned").get_health();
        if now > max_health {
            self.change_body_health_with_owner(&body, max_health - now)?;
        }
        Ok(())
    }

    fn change_body_health_with_owner(&mut self, body: &Body, delta: f32) -> BodyResult<()> {
        // Facts come from the live driving Object, never an ID lookup.
        let is_structure = self.is_structure();
        let transition = body
            .lock()
            .expect("health body poisoned")
            .change_health_for_borrowed_owner(delta, is_structure)?;
        // C++ setCorrectDamageState updates the structure footprint and pose
        // before testing the construction status and notifying its drawable.
        // The helper borrows this owner after releasing the canonical body.
        self.apply_structure_rubble_pose();
        let under_construction = self
            .get_status_bits()
            .contains(ObjectStatusMaskType::UNDER_CONSTRUCTION);
        if transition.changed_state && !under_construction {
            if let Some(drawable) = self.get_drawable() {
                crate::object::drawable::Drawable::react_to_body_damage_state_change_on_drawable(
                    &drawable,
                    transition.damage_state,
                    self,
                );
            }
            self.rebuild_body_particles_with_owner(body);
        }
        // C++ does this AFTER reaction and particles, even if health is zero.
        let effectively_dead = body.lock().expect("health body poisoned").get_health() <= 0.0;
        self.set_effectively_dead(effectively_dead);
        Ok(())
    }

    fn rebuild_body_particles_with_owner(&self, body: &Body) {
        let aflame = self
            .get_status_bits()
            .contains(ObjectStatusMaskType::AFLAME);
        let groups = body_particle_groups(aflame);
        // Capture the existing bridge once, no retained publication. Actual
        // GameClient attach/destroy only modify particle IDs, not Object state.
        let manager =
            TheParticleSystemManager::get().and_then(|manager| manager.borrowed_manager());
        // CPP selects templates before removing old systems. Do not query
        // bones for a missing actual template. Original static caching remains
        // a separate rule-load lifetime question, not a new global here.
        let groups = groups
            .into_iter()
            .map(|group| {
                let template = manager
                    .as_ref()
                    .and_then(|mgr| mgr.find_template(&group.system));
                (group, template)
            })
            .collect::<Vec<_>>();
        loop {
            let id = body
                .lock()
                .expect("body particles poisoned")
                .owner_particle_head();
            let Some(id) = id else {
                break;
            };
            if let Some(manager) = &manager {
                manager.destroy_particle_system(id);
            }
            body.lock()
                .expect("body particles poisoned")
                .remove_owner_particle_head();
        }
        let Some(manager) = manager else {
            return;
        };
        for (group, template) in groups {
            let Some(template) = template else {
                continue;
            };
            // CPP queries up to 16 bones, then limits the creation loop.
            let positions = self.get_multi_logical_bone_position(&group.prefix, 16);
            let count = group.count.max(0) as usize;
            let count = count.min(positions.len());
            let mut used = vec![false; positions.len()];
            for i in 0..count {
                let pick = game_client_random_value(0, (count - i - 1) as i32) as usize;
                let index = used
                    .iter()
                    .enumerate()
                    .filter(|(_, flag)| !**flag)
                    .nth(pick)
                    .map(|(index, _)| index)
                    .expect("available particle bone");
                used[index] = true;
                let Some(id) = manager.create_particle_system(template) else {
                    continue;
                };
                manager.set_particle_system_position(id, &positions[index]);
                manager.attach_particle_system_to_object(id, self.get_id());
                // Record each successful creation immediately, as CPP's list
                // push after attach, before the next manager callback.
                body.lock()
                    .expect("body particles poisoned")
                    .record_owner_particle(id);
            }
        }
    }
}
