//! The authored EMPUpdate runs on its receiving Object during the logic step.
use super::host_emp_pulse::emp_intended_victim_near_miss_disables;
use super::native_lifecycle::NativeLifecycleState;
use super::native_lifecycle_definition::EmpUpdateDefinition;
use super::partition_geometry::origin_in_bounding_geometry_range;
use super::{GameLogic, KindOf, ObjectId, ScriptPresentationEffects};
use game_engine::common::game_common::Relationship;
use std::sync::Arc;

enum EmpContact {
    Kill,
    Disable,
}

impl GameLogic {
    pub(super) fn tick_native_emp_with_effects(
        &mut self,
        id: ObjectId,
        index: usize,
        identity: &Arc<()>,
        definition: &EmpUpdateDefinition,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let frame = self.frame;
        let (fade_frame, die_frame, scale) = {
            let Some(object) = self.objects.get_mut(&id) else {
                return Ok(());
            };
            let Some(NativeLifecycleState::Emp(emp)) = object
                .native_lifecycle
                .as_mut()
                .and_then(|runtime| runtime.states.get_mut(index))
            else {
                return Ok(());
            };
            if !emp.active {
                return Ok(());
            }
            // EMPUpdate.cpp:120–121 changes only the Drawable's instance
            // scale. The partition and the physical footprint stay fixed.
            emp.current_scale += (emp.target_scale - emp.current_scale) * 0.05;
            object.drawable_instance_scale = emp.current_scale;
            object.visual_draw_state_revision = object.visual_draw_state_revision.wrapping_add(1);
            emp.active = frame < emp.die_frame;
            (emp.fade_frame, emp.die_frame, emp.current_scale)
        };

        #[cfg(feature = "game_client")]
        if let (Some(view), Some(object)) = (
            effects.view.as_deref_mut(),
            self.objects.get_registered(&id),
        ) {
            use game_client::core::game_client::NativeDrawableColorWrite;
            let color = if frame < fade_frame {
                Some(NativeDrawableColorWrite::Tint(saturate_rgb(
                    definition.start_color,
                    2.0,
                )))
            } else if frame == fade_frame {
                Some(NativeDrawableColorWrite::Flash {
                    color: saturate_rgb(definition.end_color, 5.0),
                    decay_frames: 9999,
                    attack_frames: die_frame.wrapping_sub(fade_frame),
                    sustain_frames: 1,
                })
            } else {
                None
            };
            view.write_emp_drawable_visual(&self.live_draw_playback_identity(object), scale, color)
                .map_err(generals_sim::GameLogicError::Configuration)?;
        }
        #[cfg(not(feature = "game_client"))]
        let _ = scale;

        if frame == fade_frame {
            self.dispatch_native_emp_with_effects(id, definition, effects)?;
        }
        if frame >= die_frame
            && self
                .objects
                .get(&id)
                .is_some_and(|object| Arc::ptr_eq(identity, &object.visual_draw_identity_token))
        {
            self.apply_owned_kill_with_effects(
                id,
                super::combat::DamageType::Unresistable,
                super::host_usa_pilot::HostDeathType::Normal,
                effects,
            )?;
        }
        Ok(())
    }

    fn dispatch_native_emp_with_effects(
        &mut self,
        id: ObjectId,
        definition: &EmpUpdateDefinition,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        use super::combat::DamageType;
        use super::host_usa_pilot::HostDeathType;

        let Some(spheroid) = self.objects.get_registered(&id) else {
            return Ok(());
        };
        let position = spheroid.get_position();
        let source_owner = spheroid.owner_player_id;
        let intended = spheroid
            .producer_id
            .and_then(|producer| {
                let object = self.objects.get_registered(&producer)?;
                self.combat_system
                    .get_projectiles()
                    .get(&producer)
                    .map_or(object.target, |projectile| projectile.target_id)
            })
            .and_then(|target| {
                let object = self.objects.get_registered(&target)?;
                Some((
                    target,
                    Arc::clone(&object.visual_draw_identity_token),
                    object.status.airborne_target,
                ))
            });
        let airborne_only = intended.as_ref().is_some_and(|(_, _, airborne)| *airborne);
        let contacts = if definition.radius > 0.0 {
            self.partition_manager
                .ids_in_radius_iterator_order(position.x, position.z, definition.radius, |raw| {
                    let victim_id = ObjectId(raw);
                    victim_id != id
                        && self
                            .objects
                            .get_registered(&victim_id)
                            .is_some_and(|victim| {
                                origin_in_bounding_geometry_range(
                                    position,
                                    victim.get_position(),
                                    victim.geometry_info(),
                                    definition.radius,
                                    false,
                                )
                            })
                })
                .into_iter()
                .filter_map(|raw| {
                    let victim_id = ObjectId(raw);
                    let victim = self.objects.get_registered(&victim_id)?;
                    Some((victim_id, Arc::clone(&victim.visual_draw_identity_token)))
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let until = self.frame.wrapping_add(definition.disabled_duration_frames);
        let mut intended_processed = false;
        for (victim_id, identity) in contacts {
            let Some(victim) = self.objects.get_registered(&victim_id) else {
                continue;
            };
            if !Arc::ptr_eq(&identity, &victim.visual_draw_identity_token)
                || !victim.is_alive()
                || (airborne_only && !victim.status.airborne_target)
            {
                continue;
            }
            let structure = victim.is_kind_of(KindOf::Structure);
            if structure
                && definition.exempts_own_buildings
                && victim.owner_player_id == source_owner
            {
                continue;
            }
            if !victim.is_kind_of(KindOf::Vehicle)
                && !structure
                && !victim.is_kind_of(KindOf::SpawnsAreTheWeapons)
            {
                continue;
            }
            let contact = if victim.is_kind_of(KindOf::Aircraft) && victim.status.airborne_target {
                if victim.is_kind_of(KindOf::EmpHardened) {
                    continue;
                }
                EmpContact::Kill
            } else {
                if structure {
                    if !victim.is_faction_structure() {
                        continue;
                    }
                } else if definition.exempts_non_structure_allies
                    && self.objects.get_registered(&id).is_some_and(|source| {
                        self.object_relationship(victim, source) == Relationship::Allies
                    })
                {
                    continue;
                }
                EmpContact::Disable
            };
            // EMPUpdate.cpp:226–274 performs the airborne kill or the
            // disable synchronously in the receiving partition visitor.
            match contact {
                EmpContact::Kill => {
                    self.apply_owned_kill_with_effects(
                        victim_id,
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        effects,
                    )?;
                }
                EmpContact::Disable => {
                    self.apply_owned_emp_disabled(victim_id, until);
                    intended_processed |= intended.as_ref().is_some_and(|(id, birth, _)| {
                        *id == victim_id && Arc::ptr_eq(birth, &identity)
                    });
                    #[cfg(feature = "game_client")]
                    if let Some(sparks) = definition.sparks.as_deref()
                        && let Some(victim) = self.objects.get_registered(&victim_id)
                        && Arc::ptr_eq(&identity, &victim.visual_draw_identity_token)
                        && effects.view.as_deref_mut().is_some_and(|view| {
                            view.accepts_emp_sparks(
                                &self.live_draw_playback_identity(victim),
                                sparks,
                            )
                        })
                    {
                        self.spawn_emp_sparks_with_template(
                            victim_id,
                            definition.disabled_duration_frames,
                            sparks,
                            definition.sparks_per_cubic_foot,
                            effects,
                        );
                    }
                }
            }
        }
        if let Some((target, identity, _)) = intended {
            let near_miss = self.objects.get_registered(&target).is_some_and(|object| {
                Arc::ptr_eq(&identity, &object.visual_draw_identity_token)
                    && object.is_alive()
                    && emp_intended_victim_near_miss_disables(
                        intended_processed,
                        object.is_kind_of(KindOf::Aircraft),
                        object.is_kind_of(KindOf::EmpHardened),
                        (object.get_position() - position).length_squared(),
                        definition.radius,
                    )
            });
            if near_miss {
                self.apply_owned_emp_disabled(target, until);
            }
        }
        Ok(())
    }
}

#[cfg(feature = "game_client")]
fn saturate_rgb(color: [f32; 3], factor: f32) -> [f32; 3] {
    color.map(|channel| channel * factor - factor * 0.5)
}
