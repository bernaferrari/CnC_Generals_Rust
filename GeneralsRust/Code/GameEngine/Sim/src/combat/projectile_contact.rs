//! Weapon.cpp contact admission and the contact search for virtual projectiles.

use crate::damage::CombatDamage;
use crate::kind_of::KindOf;
use crate::weapon::WeaponCollideMask;
use game_engine::common::game_common::Relationship;
use generals_game_domain::{ObjectId, Team};
use glam::Vec3;

/// Queries against objects in the world receiving the projectile callback.
pub trait ProjectileContactObject {
    fn object_id(&self) -> ObjectId;
    fn position(&self) -> Vec3;
    fn controller(&self) -> (Option<u32>, Team);
    fn contained_by(&self) -> Option<ObjectId>;
    fn producer(&self) -> Option<ObjectId>;
    fn is_kind_of(&self, kind: KindOf) -> bool;
    fn is_fence(&self) -> bool;
    fn has_parking_place(&self) -> bool;
    fn is_burned(&self) -> bool;
    fn is_sneaky_at(&self, frame: u32) -> bool;
    fn is_alive(&self) -> bool;
    fn is_under_construction(&self) -> bool;
    fn selection_radius(&self) -> f32;
    fn vertical_extents(&self) -> VerticalExtents;
}

/// Distances from the object's origin to its lower and upper bounds.
#[derive(Clone, Copy)]
pub struct VerticalExtents {
    pub below: f32,
    pub above: f32,
}

/// Weapon contact inputs shared by registered and virtual projectiles.
pub struct ProjectileCollision {
    pub id: ObjectId,
    pub launcher: ObjectId,
    pub intended: Option<ObjectId>,
    pub mask: WeaponCollideMask,
    pub damage_type: CombatDamage,
}

impl ProjectileCollision {
    /// WeaponTemplate::shouldProjectileCollideWith (Weapon.cpp:642–731).
    /// The partition has already confirmed geometric contact.
    pub fn accepts_object<'a, T: ProjectileContactObject + 'a>(
        &self,
        target: ObjectId,
        frame: u32,
        object: impl Fn(ObjectId) -> Option<&'a T>,
        relationship: impl FnOnce(&T, &T) -> Option<Relationship>,
    ) -> bool {
        let (Some(projectile), Some(other)) = (object(self.id), object(target)) else {
            return false;
        };
        if self.intended == Some(target) {
            return true;
        }
        if target == self.launcher
            || object(self.launcher).and_then(T::contained_by) == Some(target)
        {
            return false;
        }
        if self.ignores_burned(other) {
            return false;
        }
        if shields_parked_target(target, other, || self.intended.and_then(&object)) {
            return false;
        }
        if other.is_sneaky_at(frame) {
            return false;
        }
        let mask = self.mask;
        let matches_kind = self.accepts_structure_or_fence(projectile.controller(), other)
            || (mask.contains(WeaponCollideMask::SHRUBBERY) && other.is_kind_of(KindOf::Shrubbery))
            || (mask.contains(WeaponCollideMask::PROJECTILE)
                && other.is_kind_of(KindOf::Projectile))
            || (mask.contains(WeaponCollideMask::SMALL_MISSILES)
                && other.is_kind_of(KindOf::SmallMissile))
            || (mask.contains(WeaponCollideMask::BALLISTIC_MISSILES)
                && other.is_kind_of(KindOf::BallisticMissile));
        matches_kind
            || (mask.contains(WeaponCollideMask::ALLIES | WeaponCollideMask::ENEMIES)
                && match relationship(projectile, other) {
                    Some(Relationship::Allies) => mask.contains(WeaponCollideMask::ALLIES),
                    Some(Relationship::Enemies) => mask.contains(WeaponCollideMask::ENEMIES),
                    _ => false,
                })
    }

    /// Select the first eligible structure or fence in world registration order.
    /// A virtual shot can then hit its intended target by proximity.
    pub fn find_virtual_contact<'a, T, I>(
        &self,
        flight: VirtualProjectileContact,
        frame: u32,
        object: impl Fn(ObjectId) -> Option<&'a T>,
        registered: impl FnOnce() -> I,
    ) -> Option<ObjectId>
    where
        T: ProjectileContactObject + 'a,
        I: Iterator<Item = (&'a ObjectId, &'a T)>,
    {
        let intended = self.intended.and_then(&object);
        if projectile_collides_structures_or_fences(self.mask.bits()) {
            let launcher = object(self.launcher);
            let (owner, team) = launcher.map_or(flight.launch_controller, T::controller);
            let controller = (owner.or(flight.launch_controller.0), team);
            let launcher_container = launcher.and_then(T::contained_by);
            for (&id, other) in registered() {
                if id == self.id
                    || id == self.launcher
                    || Some(id) == self.intended
                    || Some(id) == launcher_container
                    || !other.is_alive()
                    || other.is_under_construction()
                {
                    continue;
                }
                if !self.accepts_structure_or_fence(controller, other)
                    || !flight.overlaps(other)
                    || self.ignores_burned(other)
                    || other.is_sneaky_at(frame)
                    || shields_parked_target(id, other, || intended)
                {
                    continue;
                }
                return Some(id);
            }
        }
        let target = intended?;
        (flight.position.distance(target.position()) <= 5.0).then_some(target.object_id())
    }

    fn accepts_structure_or_fence(
        &self,
        (owner, team): (Option<u32>, Team),
        other: &impl ProjectileContactObject,
    ) -> bool {
        let (other_owner, other_team) = other.controller();
        projectile_collides_with_structure_or_fence(
            self.mask.bits(),
            other.is_kind_of(KindOf::Structure),
            other.is_fence(),
            projectile_structure_same_controller(owner, other_owner, team == other_team),
        )
    }

    fn ignores_burned(&self, other: &impl ProjectileContactObject) -> bool {
        matches!(
            self.damage_type,
            CombatDamage::Flame | CombatDamage::ParticleBeam
        ) && other.is_burned()
    }
}

fn shields_parked_target<'a, T: ProjectileContactObject + 'a>(
    id: ObjectId,
    other: &T,
    intended: impl FnOnce() -> Option<&'a T>,
) -> bool {
    other.is_kind_of(KindOf::FSAirfield)
        && other.has_parking_place()
        && intended().is_some_and(|target| {
            target.producer() == Some(id) || target.contained_by() == Some(id)
        })
}

/// Pose and launch controller kept by a projectile without a registered actor.
pub struct VirtualProjectileContact {
    pub position: Vec3,
    pub vertical_extents: Option<VerticalExtents>,
    pub launch_controller: (Option<u32>, Team),
}

impl VirtualProjectileContact {
    fn overlaps(&self, other: &impl ProjectileContactObject) -> bool {
        let pos = self.position;
        let other_pos = other.position();
        let dx = pos.x - other_pos.x;
        let dz = pos.z - other_pos.z;
        if (dx * dx + dz * dz).sqrt() > other.selection_radius().max(8.0) {
            return false;
        }
        let Some(geometry) = self.vertical_extents else {
            return true;
        };
        let other_geometry = other.vertical_extents();
        let top = pos.y + geometry.above;
        let bottom = pos.y - geometry.below;
        let other_top = other_pos.y + other_geometry.above;
        let other_bottom = other_pos.y - other_geometry.below;
        top >= other_bottom && bottom <= other_top
    }
}

/// Whether a structure or a fence can intercept a virtual shot.
pub fn projectile_collides_structures_or_fences(mask: u32) -> bool {
    WeaponCollideMask::new(mask).contains(
        WeaponCollideMask::STRUCTURES
            | WeaponCollideMask::WALLS
            | WeaponCollideMask::CONTROLLED_STRUCTURES,
    )
}

/// C++ controlling-player equality. Runtime objects without a player use team identity.
pub fn projectile_structure_same_controller(
    projectile_owner: Option<u32>,
    structure_owner: Option<u32>,
    same_team: bool,
) -> bool {
    match (projectile_owner, structure_owner) {
        (Some(a), Some(b)) => a == b,
        _ => same_team,
    }
}

/// C++ Weapon.cpp:716–730 treats FenceWidth independently of structure kind and controller.
pub fn projectile_collides_with_structure_or_fence(
    mask: u32,
    is_structure: bool,
    is_fence: bool,
    same_controller: bool,
) -> bool {
    let mask = WeaponCollideMask::new(mask);
    let structure_bit = if same_controller {
        WeaponCollideMask::CONTROLLED_STRUCTURES
    } else {
        WeaponCollideMask::STRUCTURES
    };
    (is_structure && mask.contains(structure_bit))
        || (is_fence && mask.contains(WeaponCollideMask::WALLS))
}
