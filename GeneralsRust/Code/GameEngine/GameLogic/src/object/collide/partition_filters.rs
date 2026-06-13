//! PartitionFilter implementations matching C++ PartitionManager.cpp filter classes.
//!
//! Each filter struct implements the `PartitionFilter` trait from partition_manager.rs,
//! providing the `allow()` method used during spatial queries to accept or reject objects.

use super::collision_geometry::{CollideInfo, GeometryInfo};
use super::{Coord3D, GameObject, ObjectId, ObjectStatusMask};
use crate::action_manager::{self, CanEnterType};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::types::ControlBarInterface;
use crate::common::{
    CommandSourceType, DisabledType, KindOf, KindOfMaskType, ObjectShroudStatus,
    ObjectStatusMaskType, ObjectStatusTypes, PlayerId, Relationship, INVALID_ID, KIND_OF_MASK_NONE,
};
use crate::player::ThePlayerList;

// ---------------------------------------------------------------------------
// PartitionFilterIsFlying
// ---------------------------------------------------------------------------

/// Reject any objects that are not currently flying.
/// Matches C++ PartitionFilterIsFlying.
pub struct PartitionFilterIsFlying;

impl PartitionFilterIsFlying {
    pub fn new() -> Self {
        Self
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterIsFlying {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        obj.is_using_airborne_locomotor()
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterIsFlying"
    }
}

impl Default for PartitionFilterIsFlying {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterWouldCollide
// ---------------------------------------------------------------------------

/// Accept/reject objects based on whether they would collide with a given geometry.
/// Matches C++ PartitionFilterWouldCollide.
pub struct PartitionFilterWouldCollide {
    position: Coord3D,
    geometry: GeometryInfo,
    angle: f32,
    desired: bool,
}

impl PartitionFilterWouldCollide {
    pub fn new(position: Coord3D, geometry: GeometryInfo, angle: f32, desired: bool) -> Self {
        Self {
            position,
            geometry,
            angle,
            desired,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterWouldCollide {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let obj_pos = obj.get_position();
        let obj_orientation = obj.get_orientation();
        let Some(obj_handle) = obj.as_object_handle() else {
            return false;
        };
        let Ok(obj_guard) = obj_handle.read() else {
            return false;
        };

        let geom = obj_guard.get_geometry_info();
        let dx = geom.bounds.max.x - geom.bounds.min.x;
        let dy = geom.bounds.max.y - geom.bounds.min.y;
        let dz = geom.bounds.max.z - geom.bounds.min.z;
        let radius = (dx.max(dy) * 0.5).max(0.01);
        let height = dz.max(0.01);
        let is_small = geom.is_small;

        let obj_geom = match obj_guard.get_template_geometry_type() {
            Some(game_engine::system::geometry::GeometryType::Sphere) => {
                GeometryInfo::new_sphere(radius, is_small)
            }
            Some(game_engine::system::geometry::GeometryType::Box) => {
                GeometryInfo::new_box(dx.max(0.01), dy.max(0.01), is_small)
            }
            Some(game_engine::system::geometry::GeometryType::Cylinder) => {
                GeometryInfo::new_cylinder(radius, height, is_small)
            }
            None => {
                if height <= radius * 0.5 {
                    GeometryInfo::new_sphere(radius, is_small)
                } else {
                    GeometryInfo::new_cylinder(radius, height, is_small)
                }
            }
        };
        let obj_height = obj_geom.get_max_height_above_position();

        let this_info = CollideInfo::new(self.position, self.geometry.clone(), self.angle);
        let that_info = CollideInfo::new(obj_pos, obj_geom, obj_orientation);

        // Z collision check
        let this_height = self.geometry.get_max_height_above_position();
        let z_ok = this_info.position.z + this_height >= that_info.position.z
            && this_info.position.z <= that_info.position.z + obj_height;

        let does_collide = if z_ok {
            super::collision_geometry::collision_test(&this_info, &that_info, None)
        } else {
            false
        };

        does_collide == self.desired
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterWouldCollide"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterSamePlayer
// ---------------------------------------------------------------------------

/// Reject any objects that are not controlled by the same player.
/// Matches C++ PartitionFilterSamePlayer.
pub struct PartitionFilterSamePlayer {
    player_id: PlayerId,
}

impl PartitionFilterSamePlayer {
    pub fn new(player_id: PlayerId) -> Self {
        Self { player_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterSamePlayer {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        obj.get_controlling_player() == self.player_id
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterSamePlayer"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRelationship
// ---------------------------------------------------------------------------

/// Reject objects that don't match the specified relationship flags.
/// Matches C++ PartitionFilterRelationship.
///
/// Use the `RELATIONSHIP_ALLOW_*` constants to construct flags:
/// `ALLOW_ALLIES`, `ALLOW_ENEMIES`, `ALLOW_NEUTRAL`.
pub struct PartitionFilterRelationship {
    /// Object whose relationships are being tested.
    obj_id: ObjectId,
    /// Bitmask of allowed relationship types (1<<ALLIES, 1<<ENEMIES, 1<<NEUTRAL).
    flags: u32,
}

/// Relationship allow flag constants (matching C++ PartitionFilterRelationship::RelationshipAllowTypes).
pub const RELATIONSHIP_ALLOW_ALLIES: u32 = 1 << (Relationship::Allies as u32);
pub const RELATIONSHIP_ALLOW_ENEMIES: u32 = 1 << (Relationship::Enemies as u32);
pub const RELATIONSHIP_ALLOW_NEUTRAL: u32 = 1 << (Relationship::Neutral as u32);

impl PartitionFilterRelationship {
    pub fn new(obj_id: ObjectId, flags: u32) -> Self {
        Self { obj_id, flags }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRelationship {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        // Resolve the source object and compute the relationship.
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                if let Some(other_handle) = obj.as_object_handle() {
                    if let Ok(other_guard) = other_handle.read() {
                        let rel = src_guard.relationship_to(&other_guard);
                        let bit = 1u32.checked_shl(rel as u32).unwrap_or(0);
                        return (self.flags & bit) != 0;
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRelationship"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterAcceptOnTeam
// ---------------------------------------------------------------------------

/// Reject objects that are not on the specified team.
/// Matches C++ PartitionFilterAcceptOnTeam.
pub struct PartitionFilterAcceptOnTeam {
    team_id: ObjectId, // Using ObjectId as a proxy for team identifier
}

impl PartitionFilterAcceptOnTeam {
    pub fn new(team_id: ObjectId) -> Self {
        Self { team_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterAcceptOnTeam {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                if let Some(team) = guard.get_team() {
                    if let Ok(team_guard) = team.read() {
                        return team_guard.get_id() == self.team_id;
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterAcceptOnTeam"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterAcceptOnSquad
// ---------------------------------------------------------------------------

/// Reject objects that are not on the specified squad (or are dead).
/// Matches C++ PartitionFilterAcceptOnSquad.
pub struct PartitionFilterAcceptOnSquad {
    squad_id: ObjectId, // Using ObjectId as proxy for squad identifier
}

impl PartitionFilterAcceptOnSquad {
    pub fn new(squad_id: ObjectId) -> Self {
        Self { squad_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterAcceptOnSquad {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if obj.is_effectively_dead() {
            return false;
        }

        if self.squad_id == INVALID_ID {
            return true;
        }

        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                if let Some(team_id) = guard.get_team_id() {
                    // PARITY_NOTE: Squad identity is not yet ported separately from Team.
                    // Until Squad APIs exist on Object/GameObject, use team id as best available
                    // membership discriminator while still honoring this filter's squad_id.
                    return team_id == self.squad_id;
                }
            }
        }

        true
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterAcceptOnSquad"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterLineOfSight
// ---------------------------------------------------------------------------

/// Reject objects not within clear line-of-sight of the given object.
/// Matches C++ PartitionFilterLineOfSight.
#[allow(dead_code)]
pub struct PartitionFilterLineOfSight {
    obj_id: ObjectId,
}

impl PartitionFilterLineOfSight {
    #[allow(dead_code)]
    pub fn new(obj_id: ObjectId) -> Self {
        Self { obj_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterLineOfSight {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let Some(source_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id)
        else {
            return false;
        };
        let Ok(source_guard) = source_handle.read() else {
            return false;
        };

        let source_raw_pos = source_guard.get_position();
        let source_pos = Coord3D::new(source_raw_pos.x, source_raw_pos.y, source_raw_pos.z);
        let target_pos = obj.get_position();
        let target_id = obj.as_object_handle().as_ref().map(|_| obj.get_id());

        super::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            Some(self.obj_id),
            &source_pos,
            target_id,
            &target_pos,
        )
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterLineOfSight"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterPossibleToAttack
// ---------------------------------------------------------------------------

/// Accept only objects that the source can possibly attack.
/// Matches C++ PartitionFilterPossibleToAttack.
pub struct PartitionFilterPossibleToAttack {
    obj_id: ObjectId,
    attack_type: AbleToAttackType,
    command_source: CommandSourceType,
}

impl PartitionFilterPossibleToAttack {
    pub fn new(
        obj_id: ObjectId,
        attack_type: AbleToAttackType,
        command_source: CommandSourceType,
    ) -> Self {
        Self {
            obj_id,
            attack_type,
            command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPossibleToAttack {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                if let Some(other_handle) = obj.as_object_handle() {
                    if let Ok(other_guard) = other_handle.read() {
                        let result = src_guard.get_able_to_attack_specific_object(
                            self.attack_type,
                            &other_guard,
                            self.command_source,
                        );
                        return matches!(
                            result,
                            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                        );
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPossibleToAttack"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterPossibleToEnter
// ---------------------------------------------------------------------------

/// Accept only objects that the source can possibly enter.
/// Matches C++ PartitionFilterPossibleToEnter.
pub struct PartitionFilterPossibleToEnter {
    obj_id: ObjectId,
    command_source: CommandSourceType,
}

impl PartitionFilterPossibleToEnter {
    pub fn new(obj_id: ObjectId, command_source: CommandSourceType) -> Self {
        Self {
            obj_id,
            command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPossibleToEnter {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                if let Some(other_handle) = obj.as_object_handle() {
                    if let Ok(other_guard) = other_handle.read() {
                        return action_manager::TheActionManager::can_enter_object(
                            &src_guard,
                            &other_guard,
                            self.command_source,
                            CanEnterType::DontCheckCapacity,
                        );
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPossibleToEnter"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterPossibleToHijack
// ---------------------------------------------------------------------------

/// Accept only objects that the source can possibly hijack.
/// Matches C++ PartitionFilterPossibleToHijack.
pub struct PartitionFilterPossibleToHijack {
    obj_id: ObjectId,
    command_source: CommandSourceType,
}

impl PartitionFilterPossibleToHijack {
    pub fn new(obj_id: ObjectId, command_source: CommandSourceType) -> Self {
        Self {
            obj_id,
            command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPossibleToHijack {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                if let Some(other_handle) = obj.as_object_handle() {
                    if let Ok(other_guard) = other_handle.read() {
                        return action_manager::TheActionManager::can_hijack_vehicle(
                            &src_guard,
                            &other_guard,
                            self.command_source,
                        );
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPossibleToHijack"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterLastAttackedBy
// ---------------------------------------------------------------------------

/// Accept only the last object that attacked the source.
/// Matches C++ PartitionFilterLastAttackedBy.
pub struct PartitionFilterLastAttackedBy {
    last_attacked_by: ObjectId,
}

impl PartitionFilterLastAttackedBy {
    pub fn new(obj_id: ObjectId) -> Self {
        let last_attacked_by =
            if let Some(handle) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(guard) = handle.read() {
                    if let Some(body) = guard.get_body_module() {
                        if let Ok(body_guard) = body.lock() {
                            body_guard
                                .get_last_damage_info()
                                .map(|info| info.source_id)
                                .unwrap_or(INVALID_ID)
                        } else {
                            INVALID_ID
                        }
                    } else {
                        INVALID_ID
                    }
                } else {
                    INVALID_ID
                }
            } else {
                INVALID_ID
            };
        Self { last_attacked_by }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterLastAttackedBy {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        obj.get_id() == self.last_attacked_by
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterLastAttackedBy"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterAcceptByObjectStatus
// ---------------------------------------------------------------------------

/// Accept objects whose status bits match the required masks.
/// Matches C++ PartitionFilterAcceptByObjectStatus.
pub struct PartitionFilterAcceptByObjectStatus {
    must_be_set: ObjectStatusMask,
    must_be_clear: ObjectStatusMask,
}

impl PartitionFilterAcceptByObjectStatus {
    pub fn new(must_be_set: ObjectStatusMask, must_be_clear: ObjectStatusMask) -> Self {
        Self {
            must_be_set,
            must_be_clear,
        }
    }

    /// Construct from raw bitmask values.
    pub fn from_bits(must_be_set: u64, must_be_clear: u64) -> Self {
        Self {
            must_be_set: ObjectStatusMask(must_be_set),
            must_be_clear: ObjectStatusMask(must_be_clear),
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterAcceptByObjectStatus {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let status = obj.get_status_bits();
        status.test_for_all(self.must_be_set) && !(status.test_for_any(self.must_be_clear))
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterAcceptByObjectStatus"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRejectByObjectStatus
// ---------------------------------------------------------------------------

/// Reject objects whose status bits match the required masks.
/// Matches C++ PartitionFilterRejectByObjectStatus.
pub struct PartitionFilterRejectByObjectStatus {
    must_be_set: ObjectStatusMask,
    must_be_clear: ObjectStatusMask,
}

impl PartitionFilterRejectByObjectStatus {
    pub fn new(must_be_set: ObjectStatusMask, must_be_clear: ObjectStatusMask) -> Self {
        Self {
            must_be_set,
            must_be_clear,
        }
    }

    /// Construct from raw bitmask values.
    pub fn from_bits(must_be_set: u64, must_be_clear: u64) -> Self {
        Self {
            must_be_set: ObjectStatusMask(must_be_set),
            must_be_clear: ObjectStatusMask(must_be_clear),
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRejectByObjectStatus {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let status = obj.get_status_bits();
        !(status.test_for_all(self.must_be_set) && !(status.test_for_any(self.must_be_clear)))
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRejectByObjectStatus"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterStealthedAndUndetected
// ---------------------------------------------------------------------------

/// Accept or reject stealthed-and-undetected objects (based on `allow` flag).
/// Matches C++ PartitionFilterStealthedAndUndetected.
pub struct PartitionFilterStealthedAndUndetected {
    #[allow(dead_code)]
    obj_id: ObjectId,
    allow: bool,
}

impl PartitionFilterStealthedAndUndetected {
    pub fn new(obj_id: ObjectId, allow: bool) -> Self {
        Self { obj_id, allow }
    }

    fn source_player(&self) -> Option<std::sync::Arc<std::sync::RwLock<crate::player::Player>>> {
        crate::object::registry::OBJECT_REGISTRY
            .get_object(self.obj_id)
            .and_then(|source| source.read().ok()?.get_controlling_player())
    }

    fn disguised_as_enemy_for_source(&self, target: &crate::object::Object) -> Option<bool> {
        if !target.test_status(ObjectStatusTypes::Disguised) {
            return None;
        }

        let disguised_player_index = target
            .get_behavior_modules()
            .into_iter()
            .filter_map(|module| module.lock().ok()?.get_disguised_player_index())
            .next();
        let Some(disguised_player_index) = disguised_player_index else {
            return None;
        };

        let Some(source_player) = self.source_player() else {
            return None;
        };
        let Ok(source_player) = source_player.read() else {
            return None;
        };

        let other_player = ThePlayerList()
            .read()
            .ok()
            .and_then(|list| list.get_player(disguised_player_index).cloned());
        let Some(other_team) =
            other_player.and_then(|player| player.read().ok()?.get_default_team())
        else {
            return None;
        };
        let Ok(other_team) = other_team.read() else {
            return None;
        };

        Some(source_player.get_relationship_with_team(&other_team) == Relationship::Enemies)
    }

    fn neutral_container_hides_enemy_stealth_units(&self, target: &crate::object::Object) -> bool {
        let Some(contain) = target.get_contain() else {
            return false;
        };
        let Ok(contain) = contain.lock() else {
            return false;
        };
        let contain_count = contain.get_contain_count();
        if contain_count == 0 || contain.get_stealth_units_contained() != contain_count {
            return false;
        }

        let Some(first_member) = contain
            .get_contained_objects()
            .first()
            .and_then(|id| crate::helpers::TheGameLogic::find_object_by_id(*id))
        else {
            return false;
        };
        if first_member
            .read()
            .ok()
            .map(|guard| guard.test_status(ObjectStatusTypes::Detected))
            .unwrap_or(true)
        {
            return false;
        }

        let Some(source_player) = self.source_player() else {
            return false;
        };
        let Ok(source_player_guard) = source_player.read() else {
            return false;
        };
        let Some(victim_player) =
            contain.get_apparent_controlling_player(Some(&source_player_guard))
        else {
            return false;
        };
        let Some(victim_team) = victim_player
            .read()
            .ok()
            .and_then(|player| player.get_default_team())
        else {
            return false;
        };
        let Ok(victim_team) = victim_team.read() else {
            return false;
        };

        source_player_guard.get_relationship_with_team(&victim_team) == Relationship::Enemies
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterStealthedAndUndetected {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let stealthed = guard.test_status(ObjectStatusTypes::Stealthed);
                let detected = guard.test_status(ObjectStatusTypes::Detected);

                if stealthed && !detected {
                    if !guard.is_kind_of(KindOf::Disguiser) {
                        return self.allow;
                    }
                    if let Some(disguised_as_enemy) = self.disguised_as_enemy_for_source(&guard) {
                        return if disguised_as_enemy {
                            !self.allow
                        } else {
                            self.allow
                        };
                    }
                    return !self.allow;
                }

                if self.neutral_container_hides_enemy_stealth_units(&guard) {
                    return self.allow;
                }
            }
        }
        !self.allow
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterStealthedAndUndetected"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterAcceptByKindOf
// ---------------------------------------------------------------------------

/// Accept objects that match the required/forbidden KindOf masks.
/// Matches C++ PartitionFilterAcceptByKindOf.
pub struct PartitionFilterAcceptByKindOf {
    must_be_set: KindOfMaskType,
    must_be_clear: KindOfMaskType,
}

impl PartitionFilterAcceptByKindOf {
    pub fn new(must_be_set: KindOfMaskType, must_be_clear: KindOfMaskType) -> Self {
        Self {
            must_be_set,
            must_be_clear,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterAcceptByKindOf {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return guard.is_kind_of_multi(self.must_be_set, self.must_be_clear);
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterAcceptByKindOf"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRejectByKindOf
// ---------------------------------------------------------------------------

/// Reject objects that match the required/forbidden KindOf masks.
/// Matches C++ PartitionFilterRejectByKindOf.
pub struct PartitionFilterRejectByKindOf {
    must_be_set: KindOfMaskType,
    must_be_clear: KindOfMaskType,
}

impl PartitionFilterRejectByKindOf {
    pub fn new(must_be_set: KindOfMaskType, must_be_clear: KindOfMaskType) -> Self {
        Self {
            must_be_set,
            must_be_clear,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRejectByKindOf {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return !guard.is_kind_of_multi(self.must_be_set, self.must_be_clear);
            }
        }
        true // If we can't check, don't reject
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRejectByKindOf"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRejectBehind
// ---------------------------------------------------------------------------

/// Reject objects that are "behind" the given object (3D dot-product check).
/// Matches C++ PartitionFilterRejectBehind.
pub struct PartitionFilterRejectBehind {
    obj_id: ObjectId,
}

impl PartitionFilterRejectBehind {
    pub fn new(obj_id: ObjectId) -> Self {
        Self { obj_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRejectBehind {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                let src_pos = src_guard.get_position();
                let other_pos = obj.get_position();

                let dir = src_guard
                    .get_transform_matrix()
                    .x_axis
                    .truncate()
                    .normalize_or_zero();

                let v_x = other_pos.x - src_pos.x;
                let v_y = other_pos.y - src_pos.y;
                let v_z = other_pos.z - src_pos.z;

                let dot = dir.x * v_x + dir.y * v_y + dir.z * v_z;
                return dot > 0.0;
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRejectBehind"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterAlive
// ---------------------------------------------------------------------------

/// Accept only living (non-effectively-dead) objects.
/// Matches C++ PartitionFilterAlive.
pub struct PartitionFilterAlive;

impl PartitionFilterAlive {
    pub fn new() -> Self {
        Self
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterAlive {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        !obj.is_effectively_dead()
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterAlive"
    }
}

impl Default for PartitionFilterAlive {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterSameMapStatus
// ---------------------------------------------------------------------------

/// If the source is on the map, reject off-map objects (and vice versa).
/// Matches C++ PartitionFilterSameMapStatus.
pub struct PartitionFilterSameMapStatus {
    obj_id: ObjectId,
}

impl PartitionFilterSameMapStatus {
    pub fn new(obj_id: ObjectId) -> Self {
        Self { obj_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterSameMapStatus {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(src_handle) = crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id) {
            if let Ok(src_guard) = src_handle.read() {
                let src_off_map = src_guard.is_off_map();
                if let Some(other_handle) = obj.as_object_handle() {
                    if let Ok(other_guard) = other_handle.read() {
                        return other_guard.is_off_map() == src_off_map;
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterSameMapStatus"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterOnMap
// ---------------------------------------------------------------------------

/// Accept only objects that are on the map (not off-map).
/// Matches C++ PartitionFilterOnMap.
pub struct PartitionFilterOnMap;

impl PartitionFilterOnMap {
    pub fn new() -> Self {
        Self
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterOnMap {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return !guard.is_off_map();
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterOnMap"
    }
}

impl Default for PartitionFilterOnMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRejectBuildings
// ---------------------------------------------------------------------------

/// Reject buildings, unless they can attack or are owned by the enemy AI.
/// Matches C++ PartitionFilterRejectBuildings.
pub struct PartitionFilterRejectBuildings {
    obj_id: ObjectId,
    acquire_enemies: bool,
}

impl PartitionFilterRejectBuildings {
    pub fn new(obj_id: ObjectId) -> Self {
        let acquire_enemies =
            if let Some(handle) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(guard) = handle.read() {
                    // Query ThePlayerList to check if the controlling player is human (C++ uses
                    // Player::getPlayerType() == PLAYER_TYPE_COMPUTER). This replaces the
                    // previous hardcoded `player_id != 0` approximation.
                    if let Some(pid) = guard.get_player_id() {
                        if let Ok(list) = ThePlayerList().read() {
                            if let Some(player_arc) = list.get_player(pid.0 as i32) {
                                if let Ok(player) = player_arc.read() {
                                    player.get_player_type() != crate::player::PlayerType::Human
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

        Self {
            obj_id,
            acquire_enemies,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRejectBuildings {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(other_handle) = obj.as_object_handle() {
            if let Ok(other_guard) = other_handle.read() {
                // Non-structures always pass
                if !other_guard.is_kind_of(KindOf::Structure) {
                    return true;
                }

                if let Some(src_handle) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id)
                {
                    if let Ok(src_guard) = src_handle.read() {
                        let Some(my_player) = src_guard.get_controlling_player() else {
                            return false;
                        };

                        let Ok(my_guard) = my_player.read() else {
                            return false;
                        };
                        let other_player = other_guard
                            .get_contain()
                            .and_then(|contain| {
                                contain.lock().ok().and_then(|guard| {
                                    guard.get_apparent_controlling_player(Some(&my_guard))
                                })
                            })
                            .or_else(|| other_guard.get_controlling_player());

                        let Some(other_default_team) = other_player.and_then(|player| {
                            player
                                .read()
                                .ok()
                                .and_then(|guard| guard.get_default_team())
                        }) else {
                            return false;
                        };

                        let rel = other_default_team
                            .read()
                            .ok()
                            .map(|other_team_guard| {
                                my_guard.get_relationship_with_team(&other_team_guard)
                            })
                            .unwrap_or(Relationship::Neutral);

                        if rel != Relationship::Enemies {
                            return false;
                        }

                        // Computer players auto-acquire enemy buildings
                        if self.acquire_enemies {
                            return true;
                        }

                        // Don't reject faction base defenses.
                        if other_guard.is_kind_of(KindOf::FSBaseDefense) {
                            return true;
                        }

                        // Don't reject garrisoned buildings that can attack
                        if other_guard.get_contain().is_some() && other_guard.is_able_to_attack() {
                            return true;
                        }
                    }
                }

                return false;
            }
        }
        true
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRejectBuildings"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterInsignificantBuildings
// ---------------------------------------------------------------------------

/// Accept/reject insignificant buildings.
/// Matches C++ PartitionFilterInsignificantBuildings.
pub struct PartitionFilterInsignificantBuildings {
    allow_non_buildings: bool,
    allow_insignificant: bool,
}

impl PartitionFilterInsignificantBuildings {
    pub fn new(allow_non_buildings: bool, allow_insignificant: bool) -> Self {
        Self {
            allow_non_buildings,
            allow_insignificant,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterInsignificantBuildings {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                if guard.is_structure() {
                    if guard.is_non_faction_structure() && !self.allow_insignificant {
                        if let Some(contain) = guard.get_contain() {
                            let Ok(contain_guard) = contain.lock() else {
                                return false;
                            };
                            if !contain_guard.is_garrisonable()
                                || contain_guard.get_contained_count() == 0
                            {
                                return false;
                            }
                        }
                    }
                    return true;
                } else if self.allow_non_buildings {
                    return true;
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterInsignificantBuildings"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterFreeOfFog
// ---------------------------------------------------------------------------

/// Accept only objects that are clear of fog/shroud for the given player.
/// Matches C++ PartitionFilterFreeOfFog.
pub struct PartitionFilterFreeOfFog {
    comparison_index: i32,
}

impl PartitionFilterFreeOfFog {
    pub fn new(player_index: i32) -> Self {
        Self {
            comparison_index: player_index,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterFreeOfFog {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return guard.get_shrouded_status(self.comparison_index)
                    == ObjectShroudStatus::Clear;
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterFreeOfFog"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterRepulsor
// ---------------------------------------------------------------------------

/// Accept repulsor objects (enemies, or objects flagged as repulsor).
/// Matches C++ PartitionFilterRepulsor.
pub struct PartitionFilterRepulsor {
    obj_id: ObjectId,
}

impl PartitionFilterRepulsor {
    pub fn new(obj_id: ObjectId) -> Self {
        Self { obj_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterRepulsor {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        // Don't repulse yourself
        if obj.get_id() == self.obj_id {
            return false;
        }

        if let Some(other_handle) = obj.as_object_handle() {
            if let Ok(other_guard) = other_handle.read() {
                // If flagged as repulsor, always accept
                if other_guard.test_status(ObjectStatusTypes::Repulsor) {
                    return true;
                }

                // No dead enemies
                if other_guard.is_effectively_dead() {
                    return false;
                }

                if let Some(src_handle) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(self.obj_id)
                {
                    if let Ok(src_guard) = src_handle.read() {
                        let rel = src_guard.relationship_to(&other_guard);
                        if rel != Relationship::Enemies {
                            return false;
                        }

                        // Always pay attention to buildings that can attack
                        if other_guard.is_kind_of(KindOf::Structure) {
                            return other_guard.is_able_to_attack();
                        }

                        // Inert objects are not repulsors
                        if other_guard.is_kind_of(KindOf::Immobile) {
                            return false;
                        }

                        // Only enemies that can attack
                        return other_guard.is_able_to_attack();
                    }
                }
            }
        }
        false
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterRepulsor"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterIrregularArea
// ---------------------------------------------------------------------------

/// Accept only objects within the given irregular (polygon) area.
/// Matches C++ PartitionFilterIrregularArea.
pub struct PartitionFilterIrregularArea {
    area: Vec<Coord3D>,
}

impl PartitionFilterIrregularArea {
    pub fn new(area: Vec<Coord3D>) -> Self {
        Self { area }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterIrregularArea {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if self.area.len() < 3 {
            return false;
        }

        let pos = obj.get_position();
        point_inside_polygon_2d(pos.x, pos.y, &self.area)
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterIrregularArea"
    }
}

/// Point-in-polygon test using the ray-casting (even-odd) algorithm.
/// Matches C++ PointInsideArea2D.
fn point_inside_polygon_2d(px: f32, py: f32, polygon: &[Coord3D]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;

        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }

    inside
}

// ---------------------------------------------------------------------------
// PartitionFilterPolygonTrigger
// ---------------------------------------------------------------------------

/// Accept only objects within the given PolygonTrigger area.
/// Matches C++ PartitionFilterPolygonTrigger.
pub struct PartitionFilterPolygonTrigger {
    /// Vertices of the polygon trigger (x, y, z), compared on x/y only.
    vertices: Vec<Coord3D>,
}

impl PartitionFilterPolygonTrigger {
    pub fn new(vertices: Vec<Coord3D>) -> Self {
        Self { vertices }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPolygonTrigger {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if self.vertices.len() < 3 {
            return false;
        }

        let pos = obj.get_position();
        point_inside_polygon_2d(pos.x, pos.y, &self.vertices)
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPolygonTrigger"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterPlayer
// ---------------------------------------------------------------------------

/// Accept/reject objects that are (or are not) controlled by the given player.
/// Matches C++ PartitionFilterPlayer.
pub struct PartitionFilterPlayer {
    player_id: PlayerId,
    match_flag: bool,
}

impl PartitionFilterPlayer {
    pub fn new(player_id: PlayerId, match_flag: bool) -> Self {
        Self {
            player_id,
            match_flag,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPlayer {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        (obj.get_controlling_player() == self.player_id) == self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPlayer"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterPlayerAffiliation
// ---------------------------------------------------------------------------

/// Accept/reject objects based on player affiliation (enemy/ally/neutral).
/// Matches C++ PartitionFilterPlayerAffiliation.
pub struct PartitionFilterPlayerAffiliation {
    player_id: PlayerId,
    match_flag: bool,
    /// Bitmask of allowed affiliations (ALLOW_ENEMIES, ALLOW_ALLIES, ALLOW_NEUTRAL).
    affiliation: u32,
}

/// Affiliation allow flags (matching C++ AllowPlayerRelationship).
pub const AFFILIATION_ALLOW_ENEMIES: u32 = 0x01;
pub const AFFILIATION_ALLOW_NEUTRAL: u32 = 0x02;
pub const AFFILIATION_ALLOW_ALLIES: u32 = 0x04;

impl PartitionFilterPlayerAffiliation {
    pub fn new(player_id: PlayerId, affiliation: u32, match_flag: bool) -> Self {
        Self {
            player_id,
            match_flag,
            affiliation,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPlayerAffiliation {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let Some(other_handle) = obj.as_object_handle() else {
            return !self.match_flag;
        };
        let Ok(other_guard) = other_handle.read() else {
            return !self.match_flag;
        };

        let rel = compute_player_affiliation(self.player_id, &other_guard);
        let matches = match rel {
            Relationship::Enemies => self.affiliation & AFFILIATION_ALLOW_ENEMIES != 0,
            Relationship::Neutral => self.affiliation & AFFILIATION_ALLOW_NEUTRAL != 0,
            Relationship::Allies => self.affiliation & AFFILIATION_ALLOW_ALLIES != 0,
        };

        if matches || other_guard.get_player_id() == Some(self.player_id) {
            return self.match_flag;
        }

        !self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPlayerAffiliation"
    }
}

/// Compute the relationship of a player to an object.
/// Matches C++ Player::getRelationship(other->getTeam()).
fn compute_player_affiliation(player_id: PlayerId, obj: &crate::object::Object) -> Relationship {
    let Some(team_arc) = obj.get_team() else {
        return Relationship::Neutral;
    };
    let Ok(team_guard) = team_arc.read() else {
        return Relationship::Neutral;
    };
    let Ok(player_list) = ThePlayerList().read() else {
        return Relationship::Neutral;
    };
    let Some(player_arc) = player_list.get_player(player_id.0.into()) else {
        return Relationship::Neutral;
    };
    let Ok(player_guard) = player_arc.read() else {
        return Relationship::Neutral;
    };

    player_guard.get_relationship_with_team(&team_guard)
}

// ---------------------------------------------------------------------------
// PartitionFilterThing
// ---------------------------------------------------------------------------

/// Accept/reject objects matching a specific ThingTemplate name.
/// Matches C++ PartitionFilterThing.
pub struct PartitionFilterThing {
    template_name: String,
    match_flag: bool,
}

impl PartitionFilterThing {
    pub fn new(template_name: String, match_flag: bool) -> Self {
        Self {
            template_name,
            match_flag,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterThing {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let is_match = guard.get_template_name() == self.template_name;
                return is_match == self.match_flag;
            }
        }
        !self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterThing"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterGarrisonable
// ---------------------------------------------------------------------------

/// Accept/reject objects that can/cannot be garrisoned.
/// Matches C++ PartitionFilterGarrisonable.
pub struct PartitionFilterGarrisonable {
    match_flag: bool,
}

impl PartitionFilterGarrisonable {
    pub fn new(match_flag: bool) -> Self {
        Self { match_flag }
    }
}

fn object_has_garrisonable_contain(obj: &crate::object::Object) -> bool {
    let Some(contain) = obj.get_contain() else {
        return false;
    };
    let Ok(contain_guard) = contain.lock() else {
        return false;
    };
    contain_guard.is_garrisonable()
}

impl super::partition_manager::PartitionFilter for PartitionFilterGarrisonable {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let garrisonable = object_has_garrisonable_contain(&guard);
                return garrisonable == self.match_flag;
            }
        }
        !self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterGarrisonable"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterGarrisonableByPlayer
// ---------------------------------------------------------------------------

/// Accept/reject objects that the specified player can/cannot garrison.
/// Matches C++ PartitionFilterGarrisonableByPlayer.
pub struct PartitionFilterGarrisonableByPlayer {
    player_id: PlayerId,
    match_flag: bool,
    command_source: CommandSourceType,
}

impl PartitionFilterGarrisonableByPlayer {
    pub fn new(player_id: PlayerId, match_flag: bool, command_source: CommandSourceType) -> Self {
        Self {
            player_id,
            match_flag,
            command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterGarrisonableByPlayer {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let can_garrison = obj
            .as_object_handle()
            .and_then(|handle| {
                let target_guard = handle.read().ok()?;
                let player_arc = {
                    let player_list = ThePlayerList().read().ok()?;
                    player_list.get_player(self.player_id.0.into()).cloned()?
                };
                let player_guard = player_arc.read().ok()?;
                Some(action_manager::TheActionManager::can_player_garrison(
                    &player_guard,
                    &target_guard,
                    self.command_source,
                ))
            })
            .unwrap_or(false);

        can_garrison == self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterGarrisonableByPlayer"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterUnmannedObject
// ---------------------------------------------------------------------------

/// Accept/reject objects that are/aren't unmanned.
/// Matches C++ PartitionFilterUnmannedObject.
pub struct PartitionFilterUnmannedObject {
    match_flag: bool,
}

impl PartitionFilterUnmannedObject {
    pub fn new(match_flag: bool) -> Self {
        Self { match_flag }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterUnmannedObject {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let unmanned = guard.is_disabled_by_type(DisabledType::DisabledUnmanned);
                return unmanned == self.match_flag;
            }
        }
        !self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterUnmannedObject"
    }
}

// ---------------------------------------------------------------------------
// PartitionFilterValidCommandButtonTarget
// ---------------------------------------------------------------------------

/// Accept/reject objects that can/cannot have a specific command button used on them.
/// Matches C++ PartitionFilterValidCommandButtonTarget.
pub struct PartitionFilterValidCommandButtonTarget {
    source_id: ObjectId,
    command_button_id: u32,
    match_flag: bool,
    command_source: CommandSourceType,
}

impl PartitionFilterValidCommandButtonTarget {
    pub fn new(
        source_id: ObjectId,
        command_button_id: u32,
        match_flag: bool,
        command_source: CommandSourceType,
    ) -> Self {
        Self {
            source_id,
            command_button_id,
            match_flag,
            command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterValidCommandButtonTarget {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        let mut valid_target = false;

        if let Some(target_handle) = obj.as_object_handle() {
            if let Ok(target_guard) = target_handle.read() {
                valid_target = !target_guard.is_kind_of(KindOf::Inert)
                    && !target_guard.is_kind_of(KindOf::Projectile);

                if valid_target {
                    if let Some(source_handle) =
                        crate::object::registry::OBJECT_REGISTRY.get_object(self.source_id)
                    {
                        if let Ok(source_guard) = source_handle.read() {
                            if let Some(control_bar) = crate::control_bar::get_control_bar_bridge()
                            {
                                if let Some(command_button) =
                                    control_bar.get_command_button(self.command_button_id)
                                {
                                    valid_target = command_button.is_valid_to_use_on(
                                        &source_guard,
                                        Some(&target_guard),
                                        None,
                                        self.command_source,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        valid_target == self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterValidCommandButtonTarget"
    }
}

#[cfg(test)]
mod tests {
    use super::super::partition_manager::PartitionFilter;
    use super::*;
    use crate::common::types::DefaultThingTemplate;
    use crate::common::AsciiString;
    use crate::modules::ContainModuleInterface;
    use crate::object::contain::{GarrisonContain, GarrisonContainModuleData};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use crate::player::Player;
    use crate::team::Team;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, RwLock};

    #[derive(Debug)]
    struct TestStealthContain {
        contained: Vec<crate::common::ObjectID>,
        apparent_player: Arc<RwLock<Player>>,
        stealth_units: u32,
    }

    impl ContainModuleInterface for TestStealthContain {
        fn can_contain(&self, _object_id: crate::common::ObjectID) -> bool {
            true
        }

        fn contain_object(&mut self, object_id: crate::common::ObjectID) -> Result<(), String> {
            self.contained.push(object_id);
            Ok(())
        }

        fn release_object(&mut self, object_id: crate::common::ObjectID) -> Result<(), String> {
            self.contained.retain(|id| *id != object_id);
            Ok(())
        }

        fn get_contained_objects(&self) -> &[crate::common::ObjectID] {
            &self.contained
        }

        fn get_contained_count(&self) -> usize {
            self.contained.len()
        }

        fn get_max_capacity(&self) -> usize {
            usize::MAX
        }

        fn get_stealth_units_contained(&self) -> u32 {
            self.stealth_units
        }

        fn get_apparent_controlling_player(
            &self,
            _observing_player: Option<&Player>,
        ) -> Option<Arc<RwLock<Player>>> {
            Some(Arc::clone(&self.apparent_player))
        }
    }

    fn object_with_kind_of(kind_of: &str) -> Arc<std::sync::RwLock<Object>> {
        let mut template = DefaultThingTemplate::new("TestStructure".to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), kind_of.to_string());
        template.parse_object_fields_from_ini(&fields);

        Object::new(Arc::new(template), ObjectStatusMaskType::none(), None)
            .expect("test structure object")
    }

    fn structure_object() -> Arc<std::sync::RwLock<Object>> {
        object_with_kind_of("STRUCTURE")
    }

    fn registered_object_with_kind_of(
        object_id: crate::common::ObjectID,
        kind_of: &str,
        team: Arc<RwLock<Team>>,
    ) -> Arc<std::sync::RwLock<Object>> {
        let mut template = DefaultThingTemplate::new(format!("TestObject{object_id}"));
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), kind_of.to_string());
        template.parse_object_fields_from_ini(&fields);

        Object::new_with_id(
            Arc::new(template),
            object_id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("registered test object")
    }

    fn attach_garrison_contain(object: &Arc<std::sync::RwLock<Object>>) {
        let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(
            GarrisonContain::new(
                Arc::downgrade(object),
                &GarrisonContainModuleData::default(),
            )
            .expect("garrison contain"),
        ));
        object
            .write()
            .expect("object write lock")
            .set_contain(Some(contain));
    }

    fn reset_player_list_with_players(players: &[Arc<RwLock<Player>>]) {
        let mut list = ThePlayerList().write().expect("player list write lock");
        list.clear();
        for player in players {
            list.add_player(Arc::clone(player));
        }
    }

    fn team_for_player(name: &str, id: u32, player_id: u32) -> Arc<RwLock<Team>> {
        let mut team = Team::new(AsciiString::from(name), id);
        team.set_controlling_player_id(Some(player_id));
        Arc::new(RwLock::new(team))
    }

    #[test]
    fn garrisonable_filter_uses_contain_interface_not_structure_kind() {
        let object = structure_object();

        assert!(!PartitionFilterGarrisonable::new(true).allow(&object));
        assert!(PartitionFilterGarrisonable::new(false).allow(&object));

        attach_garrison_contain(&object);

        assert!(PartitionFilterGarrisonable::new(true).allow(&object));
        assert!(!PartitionFilterGarrisonable::new(false).allow(&object));
    }

    #[test]
    fn garrisonable_by_player_rejects_when_player_is_missing() {
        let object = structure_object();
        attach_garrison_contain(&object);

        let filter = PartitionFilterGarrisonableByPlayer::new(
            PlayerId(u8::MAX),
            true,
            CommandSourceType::FromPlayer,
        );

        assert!(!filter.allow(&object));
    }

    #[test]
    fn line_of_sight_filter_rejects_missing_source() {
        OBJECT_REGISTRY.clear();
        let target = structure_object();
        let filter = PartitionFilterLineOfSight::new(93_001);

        assert!(!filter.allow(&target));
    }

    #[test]
    fn player_affiliation_uses_player_team_relationships() {
        let player0 = Arc::new(RwLock::new(Player::new(0)));
        let player1 = Arc::new(RwLock::new(Player::new(1)));
        reset_player_list_with_players(&[Arc::clone(&player0), Arc::clone(&player1)]);

        player0
            .write()
            .expect("player write lock")
            .set_player_relationship_by_index(1, Relationship::Enemies);

        let enemy_team = team_for_player("EnemyTeam", 1, 1);
        let target = structure_object();
        target
            .write()
            .expect("target write lock")
            .set_team(Some(enemy_team))
            .expect("set target team");

        let enemy_filter =
            PartitionFilterPlayerAffiliation::new(PlayerId(0), AFFILIATION_ALLOW_ENEMIES, true);
        let neutral_filter =
            PartitionFilterPlayerAffiliation::new(PlayerId(0), AFFILIATION_ALLOW_NEUTRAL, true);

        assert!(enemy_filter.allow(&target));
        assert!(!neutral_filter.allow(&target));
    }

    #[test]
    fn reject_behind_uses_transform_x_vector() {
        OBJECT_REGISTRY.clear();

        let team = team_for_player("SourceTeam", 20, 0);
        let source = registered_object_with_kind_of(92_001, "STRUCTURE", team);
        {
            let mut source_guard = source.write().expect("source write lock");
            source_guard
                .set_position(&crate::common::Coord3D::new(0.0, 0.0, 0.0))
                .expect("set source position");
            source_guard
                .set_orientation(std::f32::consts::FRAC_PI_2)
                .expect("set source orientation");
        }

        let ahead = object_with_kind_of("STRUCTURE");
        ahead
            .write()
            .expect("ahead write lock")
            .set_position(&crate::common::Coord3D::new(0.0, 10.0, 0.0))
            .expect("set ahead position");
        let behind = object_with_kind_of("STRUCTURE");
        behind
            .write()
            .expect("behind write lock")
            .set_position(&crate::common::Coord3D::new(0.0, -10.0, 0.0))
            .expect("set behind position");

        let filter =
            PartitionFilterRejectBehind::new(source.read().expect("source read lock").get_id());

        assert!(filter.allow(&ahead));
        assert!(!filter.allow(&behind));

        OBJECT_REGISTRY.unregister_object(92_001);
    }

    #[test]
    fn reject_buildings_only_accepts_enemy_fs_base_defense_for_human_sources() {
        OBJECT_REGISTRY.clear();

        let player0 = Arc::new(RwLock::new(Player::new(0)));
        let player1 = Arc::new(RwLock::new(Player::new(1)));
        let source_team = team_for_player("SourceTeam", 10, 0);
        let enemy_team = team_for_player("EnemyTeam", 11, 1);

        player0
            .write()
            .expect("player0 write lock")
            .set_player_type(crate::player::PlayerType::Human, false);
        player0
            .write()
            .expect("player0 write lock")
            .set_default_team(Some(Arc::clone(&source_team)));
        player1
            .write()
            .expect("player1 write lock")
            .set_default_team(Some(Arc::clone(&enemy_team)));
        player0
            .write()
            .expect("player0 write lock")
            .set_player_relationship_by_index(1, Relationship::Enemies);
        reset_player_list_with_players(&[Arc::clone(&player0), Arc::clone(&player1)]);

        let source = registered_object_with_kind_of(91_001, "STRUCTURE", Arc::clone(&source_team));
        let generic_defense =
            registered_object_with_kind_of(91_002, "STRUCTURE|DEFENSE", Arc::clone(&enemy_team));
        let fs_base_defense = registered_object_with_kind_of(
            91_003,
            "STRUCTURE|FS_BASE_DEFENSE",
            Arc::clone(&enemy_team),
        );

        let filter =
            PartitionFilterRejectBuildings::new(source.read().expect("source read lock").get_id());

        assert!(!filter.allow(&generic_defense));
        assert!(filter.allow(&fs_base_defense));

        OBJECT_REGISTRY.unregister_object(91_001);
        OBJECT_REGISTRY.unregister_object(91_002);
        OBJECT_REGISTRY.unregister_object(91_003);
    }

    #[test]
    fn stealthed_disguiser_without_disguise_module_is_not_hidden() {
        OBJECT_REGISTRY.clear();

        let team = team_for_player("SourceTeam", 30, 0);
        let source = registered_object_with_kind_of(93_101, "STRUCTURE", team);
        let target = object_with_kind_of("DISGUISER");
        target.write().expect("target write lock").set_status(
            ObjectStatusMaskType::STEALTHED | ObjectStatusMaskType::DISGUISED,
            true,
        );

        let filter =
            PartitionFilterStealthedAndUndetected::new(source.read().unwrap().get_id(), true);

        assert!(!filter.allow(&target));

        OBJECT_REGISTRY.unregister_object(93_101);
    }

    #[test]
    fn stealthed_container_hides_enemy_undetected_passengers() {
        OBJECT_REGISTRY.clear();

        let player0 = Arc::new(RwLock::new(Player::new(0)));
        let player1 = Arc::new(RwLock::new(Player::new(1)));
        let source_team = team_for_player("SourceTeam", 31, 0);
        let enemy_team = team_for_player("EnemyTeam", 32, 1);
        player0
            .write()
            .expect("player0 write lock")
            .set_default_team(Some(Arc::clone(&source_team)));
        player1
            .write()
            .expect("player1 write lock")
            .set_default_team(Some(Arc::clone(&enemy_team)));
        player0
            .write()
            .expect("player0 write lock")
            .set_player_relationship_by_index(1, Relationship::Enemies);
        reset_player_list_with_players(&[Arc::clone(&player0), Arc::clone(&player1)]);

        let source = registered_object_with_kind_of(93_201, "STRUCTURE", Arc::clone(&source_team));
        let container =
            registered_object_with_kind_of(93_202, "STRUCTURE", Arc::clone(&enemy_team));
        let passenger = registered_object_with_kind_of(93_203, "INFANTRY", Arc::clone(&enemy_team));
        passenger
            .write()
            .expect("passenger write lock")
            .set_status(ObjectStatusMaskType::STEALTHED, true);

        let contain: Arc<Mutex<dyn ContainModuleInterface>> =
            Arc::new(Mutex::new(TestStealthContain {
                contained: vec![93_203],
                apparent_player: Arc::clone(&player1),
                stealth_units: 1,
            }));
        container
            .write()
            .expect("container write lock")
            .set_contain(Some(contain));

        let allow_filter =
            PartitionFilterStealthedAndUndetected::new(source.read().unwrap().get_id(), true);
        let reject_filter =
            PartitionFilterStealthedAndUndetected::new(source.read().unwrap().get_id(), false);

        assert!(allow_filter.allow(&container));
        assert!(!reject_filter.allow(&container));

        OBJECT_REGISTRY.unregister_object(93_201);
        OBJECT_REGISTRY.unregister_object(93_202);
        OBJECT_REGISTRY.unregister_object(93_203);
    }
}
