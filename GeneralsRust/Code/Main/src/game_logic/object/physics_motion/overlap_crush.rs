//! C++ PhysicsUpdate::checkForOverlapCollision: bounded caller phases around
//! synchronous attemptDamage. Only the continuation crosses an Object loan.
use super::super::*;

#[derive(Debug, Clone, Copy)]
pub(in crate::game_logic) enum OverlapCrushResume {
    Start,
    AfterInitialContact,
    AfterSquish { crusher_xz: (f32, f32) },
    AfterFullCrush { crusher_xz: (f32, f32) },
}
impl OverlapCrushResume {
    pub(in crate::game_logic) fn has_started(self) -> bool {
        !matches!(self, Self::Start)
    }
}
#[derive(Debug, Clone, Copy)]
pub(in crate::game_logic) struct OverlapCrushDamage {
    pub victim: ObjectId,
    pub source: Option<ObjectId>,
    pub amount: f32,
    pub damage_type: crate::game_logic::combat::DamageType,
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
}
impl OverlapCrushDamage {
    fn new(source: ObjectId, victim: ObjectId, amount: f32) -> Self {
        Self {
            victim,
            source: Some(source),
            amount,
            damage_type: crate::game_logic::combat::DamageType::Crush,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
        }
    }
}
pub(in crate::game_logic) enum OverlapCrushStep {
    Complete(bool),
    Damage {
        request: OverlapCrushDamage,
        resume: OverlapCrushResume,
    },
}

impl Object {
    pub(in crate::game_logic) fn step_overlap_crush(
        &mut self,
        other: &mut Object,
        is_ally: bool,
        resume: OverlapCrushResume,
    ) -> OverlapCrushStep {
        use crate::game_logic::host_partition_collision_physics_residual::{
            CrushTarget, PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL, past_crush_point_residual,
        };
        match resume {
            OverlapCrushResume::AfterSquish { .. } => {
                // SquishCollide.cpp:106 follows attemptDamage even if rejected.
                other.blow_defector_cover();
                self.add_physics_overlap(other.id);
                return OverlapCrushStep::Complete(true);
            }
            OverlapCrushResume::AfterFullCrush { .. } => return OverlapCrushStep::Complete(true),
            OverlapCrushResume::Start => {
                self.ensure_crush_levels();
                other.ensure_crush_levels();
                if self.velocity_is_very_small() {
                    return OverlapCrushStep::Complete(false);
                }
                let self_crushing_other = self.can_crush_only(other, is_ally);
                let self_being_crushed = other.can_crush_only(self, is_ally);
                if self_crushing_other && self_being_crushed {
                    return OverlapCrushStep::Complete(false);
                }
                if self_being_crushed {
                    return OverlapCrushStep::Complete(true); // passive overlap
                }
                // C++ SquishCollide::onCollide is independent of TEST_CRUSH_ONLY.
                // Instant-squish when the victim has the authored module.
                if other.has_squish_collide {
                    use crate::game_logic::host_squish_collide::{
                        SQUISH_HUGE_DAMAGE, authored_crusher_geometry,
                        should_skip_squish_for_goal_ability, squish_geom_collides_with,
                        template_has_hijacker_update, velocity_toward_victim,
                    };
                    let has_hijacker = template_has_hijacker_update(&other.template_name);
                    let tnt_active = crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                        &other.template_name,
                    ) && other.ai_state
                        == crate::game_logic::AIState::SpecialAbility
                        && other.target == Some(self.id);
                    if !is_ally
                        && self.crusher_level > 0
                        && !should_skip_squish_for_goal_ability(
                            other.target,
                            self.id,
                            has_hijacker,
                            tnt_active,
                        )
                    {
                        let us = self.get_position();
                        let them = other.get_position();
                        let vel = self.movement.velocity;
                        let toward =
                            velocity_toward_victim((us.x, us.z), (them.x, them.z), (vel.x, vel.z));
                        let crusher_g = &self.thing.geometry;
                        let crusher_half_x =
                            ((crusher_g.bounds_max.x - crusher_g.bounds_min.x).abs() * 0.5)
                                .max(0.0);
                        let fallback_major = crusher_g
                            .radius
                            .max(self.selection_radius)
                            .max(crusher_half_x)
                            .max(1.0);
                        let fallback_h = (crusher_g.bounds_max.y - crusher_g.bounds_min.y)
                            .abs()
                            .max(crusher_g.radius)
                            .max(1.0);
                        let crusher_geom = authored_crusher_geometry(
                            &self.thing.template.geometry_info,
                            fallback_major,
                            fallback_h,
                        );
                        let victim_g = &other.thing.geometry;
                        let victim_h = if other.thing.template.geometry_info.authored {
                            other.thing.template.geometry_info.height.max(0.01)
                        } else {
                            (victim_g.bounds_max.y - victim_g.bounds_min.y)
                                .abs()
                                .max(victim_g.radius)
                                .max(1.0)
                        };
                        if toward
                            && squish_geom_collides_with(
                                (us.x, us.y, us.z),
                                self.get_orientation(),
                                crusher_geom,
                                (them.x, them.y, them.z),
                                other.get_orientation(),
                                victim_h,
                            )
                        {
                            // C++ SquishCollide.cpp:88-93: HUGE_DAMAGE + DEATH_CRUSHED.
                            // The victim's die pipeline then runs CrushDie::onDie
                            // (CrushDie.cpp:137-180), which stamps FRONT/BACK crushed
                            // body flags from the dealer-relative crushLocationCheck.
                            return OverlapCrushStep::Damage {
                                request: OverlapCrushDamage::new(
                                    self.id,
                                    other.id,
                                    SQUISH_HUGE_DAMAGE,
                                ),
                                resume: OverlapCrushResume::AfterSquish {
                                    crusher_xz: (us.x, us.z),
                                },
                            };
                        }
                    }
                }
                if !self_crushing_other {
                    return OverlapCrushStep::Complete(false);
                }
                // add overlap
                let oid = other.id;
                let first = self.physics_previous_overlap != Some(oid)
                    && self.physics_current_overlap != Some(oid);
                self.add_physics_overlap(oid);
                if first {
                    // 0-amount crush damage residual (DamageFX trigger only).
                    return OverlapCrushStep::Damage {
                        request: OverlapCrushDamage::new(self.id, other.id, 0.0),
                        resume: OverlapCrushResume::AfterInitialContact,
                    };
                }
            }
            OverlapCrushResume::AfterInitialContact => {}
        }
        // Every value below is read AFTER the zero-amount callback completes.
        if other.front_crushed && other.back_crushed {
            return OverlapCrushStep::Complete(true);
        }
        let us = self.get_position();
        let them = other.get_position();
        // C++ PhysicsUpdate.cpp:1488 — crush ray is unit facing, not velocity.
        let (dx_f, dz_f) = self.unit_direction_xz();
        // C++ getGeometryInfo().getMajorRadius() / 2 (PhysicsUpdate.cpp:1490).
        let geom = &other.thing.template.geometry_info;
        let major = if geom.authored {
            geom.major_radius
        } else {
            other.selection_radius.max(1.0)
        };
        let offset = major / 2.0;
        let crushee_facing = other.unit_direction_xz();
        let target = {
            use crate::game_logic::host_partition_collision_physics_residual::select_crush_target_by_perp_residual;
            select_crush_target_by_perp_residual(
                other.front_crushed,
                other.back_crushed,
                (us.x, us.z),
                (them.x, them.z),
                (dx_f, dz_f),
                crushee_facing,
                offset,
            )
        };
        if target == CrushTarget::NoCrush {
            return OverlapCrushStep::Complete(true);
        }
        let point = match target {
            CrushTarget::FrontEndCrush => (
                them.x + crushee_facing.0 * offset,
                them.z + crushee_facing.1 * offset,
            ),
            CrushTarget::BackEndCrush => (
                them.x - crushee_facing.0 * offset,
                them.z - crushee_facing.1 * offset,
            ),
            CrushTarget::TotalCrush | CrushTarget::NoCrush => (them.x, them.z),
        };
        if past_crush_point_residual((us.x, us.z), point, (dx_f, dz_f), offset) {
            // C++ applies HUGE crush damage only. CrushDie::onDie writes
            // FRONT/BACK/TOTAL from the previous (usually unset) flags.
            let crusher_xz = (us.x, us.z);
            return OverlapCrushStep::Damage {
                request: OverlapCrushDamage::new(
                    self.id,
                    other.id,
                    PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                ),
                resume: OverlapCrushResume::AfterFullCrush { crusher_xz },
            };
        }
        OverlapCrushStep::Complete(true)
    }
}

impl Object {
    pub(super) fn finish_standalone_crush_damage(&mut self, resume: OverlapCrushResume) {
        let crusher_xz = match resume {
            OverlapCrushResume::AfterSquish { crusher_xz }
            | OverlapCrushResume::AfterFullCrush { crusher_xz } => crusher_xz,
            _ => return,
        };
        if self.status.death_type != crate::game_logic::host_usa_pilot::HostDeathType::Crushed {
            return;
        }
        self.fire_crush_die_from_crusher(Some(crusher_xz));
        if let Some(declarations) = self.thing().template.native_die_declarations.as_ref() {
            let slots: Vec<_> = declarations
                .iter()
                .filter(|declaration| {
                    matches!(
                        declaration.kind,
                        crate::game_logic::native_die_definition::NativeDieKind::Crush
                    ) && declaration.gate.allows(self, self.status.death_type)
                })
                .map(|declaration| declaration.source_index)
                .collect();
            self.native_die_runtime.completed.extend(slots);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_standalone_crush_does_not_acknowledge_stale_death_type() {
        let mut crusher = Object::new(
            ThingTemplate::new("ReceiptCrusher"),
            ObjectId(98101),
            Team::USA,
        );
        crusher.crusher_level = 2;
        crusher.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
        crusher.set_position(glam::Vec3::new(15.0, 0.0, 0.2));
        let mut template = ThingTemplate::new("ReceiptVictim");
        template.add_kind_of(KindOf::Vehicle);
        template.geometry_info = crate::game_logic::HostGeometryInfo {
            geom_type: crate::game_logic::HostGeometryType::Box,
            is_small: true,
            height: 8.0,
            major_radius: 6.0,
            minor_radius: 4.0,
            authored: true,
        };
        template.native_die_declarations = Some(
            crate::game_logic::native_die_definition::bind(&[
                crate::assets::ini_parser::BehaviorModuleDefinition {
                    class_name: "CrushDie".into(),
                    module_tag: Some("CrushReceipt".into()),
                    attributes: Default::default(),
                },
            ])
            .into_boxed_slice(),
        );
        let mut victim = Object::new(template, ObjectId(98102), Team::Neutral);
        victim.crushable_level = 1;
        victim.selection_radius = 10.0;
        victim.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        victim.set_indestructible(true);
        victim.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
        let mut events = crate::game_logic::HostHealthEvents::default();
        assert!(crusher.check_for_overlap_collision(0, &mut victim, false, &mut events));
        assert!(victim.health.current > 0.0);
        assert!(!victim.front_crushed && !victim.back_crushed);
        assert!(victim.native_die_runtime.completed.is_empty());
    }
}
