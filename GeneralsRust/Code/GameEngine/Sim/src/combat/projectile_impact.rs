//! Projectile flight outcomes, detonation damage, and garrison hit selection.

use super::{
    DamageEvent,
    projectile_flight::{ProjectileFlight, ProjectileStep},
};
use crate::{
    damage::{CombatDamage, DeathOutcome, HUGE_DAMAGE_AMOUNT},
    kind_of::KindOf,
    object::ObjectStatusTypes,
    weapon::WeaponAffectsMask,
};
use game_engine::common::game_common::Relationship;
use generals_game_domain::ObjectId;
use glam::{Mat4, Vec3};

/// What to do after the projectile's update callback has advanced its flight.
pub enum ProjectileAfterUpdate {
    Wait,
    Retire,
    SearchVirtualContact,
    Detonate(ProjectileImpact),
}

impl ProjectileAfterUpdate {
    pub fn after_flight(
        step: ProjectileStep,
        position: Vec3,
        armed: bool,
        drawable_hidden: bool,
        has_registered_actor: bool,
    ) -> Self {
        match step {
            ProjectileStep::Remove => Self::Retire,
            ProjectileStep::Detonate {
                retain_actor,
                pre_physics_pose,
            } => {
                let position = pre_physics_pose.map_or(position, |pose| pose.w_axis.truncate());
                Self::Detonate(ProjectileImpact {
                    cause: ImpactCause::Flight {
                        retain_actor,
                        pre_physics_pose,
                    },
                    actor_position: position,
                    weapon_position: position,
                })
            }
            // A missile holding after target loss can still be armed and visible.
            // The partition reports registered actor contacts after movement.
            ProjectileStep::Alive if armed && !has_registered_actor => Self::SearchVirtualContact,
            ProjectileStep::Hold if armed && !drawable_hidden && !has_registered_actor => {
                Self::SearchVirtualContact
            }
            _ => Self::Wait,
        }
    }
}

/// Whether the flight already retained the actor, or a new contact did so later.
pub enum ImpactCause {
    Flight {
        retain_actor: bool,
        pre_physics_pose: Option<Mat4>,
    },
    Object(ObjectId),
    Ground,
}

/// The projectile and weapon poses can differ for a position target.
pub struct ProjectileImpact {
    pub cause: ImpactCause,
    pub actor_position: Vec3,
    pub weapon_position: Vec3,
}

impl ProjectileImpact {
    pub fn object_contact(position: Vec3, target: ObjectId) -> Self {
        Self {
            cause: ImpactCause::Object(target),
            actor_position: position,
            weapon_position: position,
        }
    }

    /// Object contact wins over proximity to a ground target.
    pub fn after_virtual_step(
        position: Vec3,
        target_position: Vec3,
        has_object_target: bool,
        contact: Option<ObjectId>,
    ) -> Option<Self> {
        if let Some(target) = contact {
            return Some(Self::object_contact(position, target));
        }
        (!has_object_target && position.distance(target_position) <= 2.0).then_some(Self {
            cause: ImpactCause::Ground,
            actor_position: position,
            weapon_position: target_position,
        })
    }

    pub fn contacted_object(&self) -> Option<ObjectId> {
        match self.cause {
            ImpactCause::Object(id) => Some(id),
            _ => None,
        }
    }

    pub fn pre_physics_pose(&self) -> Option<Mat4> {
        match self.cause {
            ImpactCause::Flight {
                pre_physics_pose, ..
            } => pre_physics_pose,
            _ => None,
        }
    }
}

/// Weapon fields that decide whether a detonation deals splash or direct damage.
pub struct ProjectileWarhead {
    pub projectile: ObjectId,
    pub launcher: ObjectId,
    pub authored_flight: bool,
    pub has_registered_actor: bool,
    pub no_damage: bool,
    pub damage: f32,
    pub damage_type: CombatDamage,
    pub death_type: DeathOutcome,
    pub damage_status_type: Option<ObjectStatusTypes>,
    pub primary_radius: f32,
    pub secondary_radius: f32,
    pub affects: WeaponAffectsMask,
}

impl ProjectileWarhead {
    /// Weapon.cpp:1280–1311. A radius-zero SUICIDE hit reaches the projectile
    /// Body before the area path would resolve damage attribution to its launcher.
    pub fn detonation_damage(
        &self,
        impact: &ProjectileImpact,
        splash: impl FnOnce(Vec3) -> DamageEvent,
    ) -> Option<DamageEvent> {
        if self.no_damage {
            return None;
        }
        let position = impact.weapon_position;
        if self.primary_radius > 0.0 || self.secondary_radius > 0.0 {
            return Some(splash(position));
        }
        let (target_id, shooter_id, damage) = if self.authored_flight {
            if !self.affects.contains(WeaponAffectsMask::KILLS_SELF) || !self.has_registered_actor {
                return None;
            }
            (self.projectile, self.projectile, HUGE_DAMAGE_AMOUNT)
        } else {
            (impact.contacted_object()?, self.launcher, self.damage)
        };
        Some(DamageEvent::Direct {
            target_id,
            position,
            damage,
            damage_type: self.damage_type,
            death_type: self.death_type,
            shooter_id,
            damage_status_type: self.damage_status_type,
        })
    }
}

/// Authored DumbProjectileBehavior and MissileAIUpdate garrison clear policy.
pub struct GarrisonHit<'a> {
    required: &'a [KindOf],
    forbidden: &'a [KindOf],
}

impl GarrisonHit<'_> {
    pub fn hits_occupant(&self, alive: bool, is_kind_of: impl Fn(KindOf) -> bool) -> bool {
        alive
            && self.required.iter().copied().all(&is_kind_of)
            && self.forbidden.iter().copied().all(|kind| !is_kind_of(kind))
    }
}

/// Visit live garrison occupants in containment order and stop at the authored
/// kill count. The receiving world immediately applies each accepted kill.
pub fn garrison_hit_casualties<W>(
    flight: &ProjectileFlight,
    world: &mut W,
    occupants: impl FnOnce(&W) -> Option<Vec<ObjectId>>,
    mut kill: impl FnMut(&mut W, ObjectId, &GarrisonHit<'_>) -> bool,
) -> Vec<ObjectId> {
    let count = flight.garrison_hit_kill_count();
    if count <= 0 {
        return Vec::new();
    }
    let Some(occupants) = occupants(world) else {
        return Vec::new();
    };
    let hit = GarrisonHit {
        required: flight.garrison_required(),
        forbidden: flight.garrison_forbidden(),
    };
    let mut killed = Vec::new();
    for id in occupants {
        if kill(world, id, &hit) {
            killed.push(id);
            if killed.len() == count as usize {
                break;
            }
        }
    }
    killed
}

/// Whether splash may affect `victim` given C++ `getRelationship` and weapon mask.
///
/// `relationship` is `curVictim->getRelationship(source)` (Weapon.cpp:1360-1372).
///
/// `producer_id` is `source->getProducerID()` (Weapon.cpp:1330-1337). When
/// `WeaponAffectsMask::SELF` is unset, C++ skips both the firer and the building
/// that produced it (War Factory, airfield, Stinger Site).
pub fn radius_damage_affects_victim(
    affects: u32,
    relationship: Relationship,
    shooter_id: ObjectId,
    victim_id: ObjectId,
    producer_id: Option<ObjectId>,
    victim_airborne: bool,
    same_template: bool,
) -> bool {
    if (shooter_id == victim_id || producer_id == Some(victim_id))
        && (affects & WeaponAffectsMask::SELF) == 0
    {
        return false;
    }
    if (affects & WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE) != 0 && victim_airborne {
        return false;
    }
    if (affects & WeaponAffectsMask::DOESNT_AFFECT_SIMILAR) != 0 && same_template {
        // C++ source->getRelationship(curVictim) == ALLIES && equivalent template.
        if relationship == Relationship::Allies {
            return false;
        }
    }
    let required_mask = match relationship {
        Relationship::Allies => WeaponAffectsMask::ALLIES,
        Relationship::Enemies => WeaponAffectsMask::ENEMIES,
        Relationship::Neutral => WeaponAffectsMask::NEUTRALS,
    };
    (affects & required_mask) != 0
}
