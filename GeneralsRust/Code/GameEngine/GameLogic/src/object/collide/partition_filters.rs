//! PartitionFilter implementations matching C++ PartitionManager.cpp filter classes.
//!
//! Each filter struct implements the `PartitionFilter` trait from partition_manager.rs,
//! providing the `allow()` method used during spatial queries to accept or reject objects.

use super::collision_geometry::{CollideInfo, GeometryInfo};
use super::{Coord3D, GameObject, ObjectId, ObjectStatusMask};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::{
    CommandSourceType, DisabledType, KindOf, KindOfMaskType, ObjectShroudStatus,
    ObjectStatusMaskType, ObjectStatusTypes, PlayerId, Relationship, INVALID_ID, KIND_OF_MASK_NONE,
};

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
        // Use the major radius as height approximation since get_max_height_above_position
        // is not yet ported on GeometryInfo.
        let obj_geom = GeometryInfo::new_sphere(5.0, false);
        let obj_height = obj_geom.get_major_radius();

        let this_info = CollideInfo::new(self.position, self.geometry.clone(), self.angle);
        let that_info = CollideInfo::new(obj_pos, obj_geom, obj_orientation);

        // Z collision check
        let this_height = self.geometry.get_major_radius();
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
pub const RELATIONSHIP_ALLOW_ENEMIES: u32 = 1 << (Relationship::Enemy as u32);
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
        // Squad membership is not yet exposed on the GameObject trait.
        // This is a placeholder that accepts non-dead objects.
        // Full implementation will hook into the Squad system once ported.
        !obj.is_effectively_dead()
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
pub struct PartitionFilterLineOfSight {
    obj_id: ObjectId,
}

impl PartitionFilterLineOfSight {
    pub fn new(obj_id: ObjectId) -> Self {
        Self { obj_id }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterLineOfSight {
    fn allow(&self, _obj: &dyn GameObject) -> bool {
        // Line-of-sight checking requires terrain data and the AI pathfinder,
        // which are not yet accessible from the GameObject trait.
        // For now, always allow -- full implementation will query terrain LOS
        // and obstacle blocking once those systems are ported.
        true
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
    _command_source: CommandSourceType,
}

impl PartitionFilterPossibleToEnter {
    pub fn new(obj_id: ObjectId, command_source: CommandSourceType) -> Self {
        Self {
            obj_id,
            _command_source: command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPossibleToEnter {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        // can_enter_object requires ActionManager integration not yet ported.
        // Approximate: accept structures that are not full and allow containment.
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return guard.is_kind_of(KindOf::Structure)
                    && !guard.is_effectively_dead()
                    && !guard.is_kind_of(KindOf::NoGarrison);
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
    _command_source: CommandSourceType,
}

impl PartitionFilterPossibleToHijack {
    pub fn new(obj_id: ObjectId, command_source: CommandSourceType) -> Self {
        Self {
            obj_id,
            _command_source: command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterPossibleToHijack {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        // Hijack checking requires ActionManager integration not yet ported.
        // Placeholder: accept objects that are vehicles and not dead.
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                return guard.is_kind_of(KindOf::Vehicle) && !guard.is_effectively_dead();
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
        // Body module / last-damage-source tracking not yet fully exposed.
        // Default to INVALID_ID.
        // TODO: Hook into BodyModule::getLastDamageInfo once ported.
        let _obj_exists = crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_some();
        let last_attacked_by = INVALID_ID;
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
    obj_id: ObjectId,
    allow: bool,
}

impl PartitionFilterStealthedAndUndetected {
    pub fn new(obj_id: ObjectId, allow: bool) -> Self {
        Self { obj_id, allow }
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
                    // Disguiser exception -- check disguise details.
                    // Full implementation will query the StealthUpdate module
                    // to check disguised state and player relationships.
                    // For now, treat disguised stealthers as not stealthed.
                }

                // Check for neutral containers holding stealth units
                // (the garrisoned-stealth edge case from C++).
                // Full implementation will hook into ContainModuleInterface.
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
                let angle = src_guard.get_orientation();
                let other_pos = obj.get_position();

                // Compute facing direction from orientation angle (2D approximation)
                let dir_x = angle.cos();
                let dir_y = angle.sin();

                let v_x = other_pos.x - src_pos.x;
                let v_y = other_pos.y - src_pos.y;

                let dot = dir_x * v_x + dir_y * v_y;
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
                    // Check if the player is a computer player.
                    // get_player_type() not yet ported; approximate by checking
                    // if the player ID is not the human player (player 0).
                    guard.get_player_id().map(|pid| pid.0 != 0).unwrap_or(false)
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
                        let rel = src_guard.relationship_to(&other_guard);

                        if rel != Relationship::Enemy {
                            return false;
                        }

                        // Computer players auto-acquire enemy buildings
                        if self.acquire_enemies {
                            return true;
                        }

                        // Don't reject base defense-like structures
                        if other_guard.is_kind_of(KindOf::FSBarracks)
                            || other_guard.is_kind_of(KindOf::FSWarfactory)
                            || other_guard.is_kind_of(KindOf::FSPower)
                            || other_guard.is_kind_of(KindOf::Defense)
                        {
                            return true;
                        }

                        // Don't reject garrisoned buildings that can attack
                        if other_guard.is_able_to_attack() {
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
                    // is_non_faction_structure not yet ported; approximate
                    // by checking for TechBuilding or Civilian kinds.
                    let is_non_faction = guard.is_kind_of(KindOf::TechBuilding)
                        || guard.is_kind_of(KindOf::Civilian);

                    if is_non_faction && !self.allow_insignificant {
                        // Check if it has a garrisonable contain with units inside.
                        // ContainModule not yet exposed here; approximate by
                        // checking if the building can attack (which implies occupants).
                        if !guard.is_able_to_attack() {
                            return false;
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
                        if rel != Relationship::Enemy {
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
        // We need the relationship from our player to the other object.
        // Use the GameObject trait's get_relationship which returns a relationship.
        // Note: this requires a source object context, so we approximate
        // by checking if the other object's team relationship to our player
        // matches one of the affiliation flags.

        // Check if same player first
        if obj.get_controlling_player() == self.player_id {
            return self.match_flag;
        }

        // Use the object-to-object relationship via the registry
        // to compute the player-level relationship.
        if let Some(other_handle) = obj.as_object_handle() {
            if let Ok(other_guard) = other_handle.read() {
                // Try to find a player-owned object to compute relationship
                // For simplicity, use the object's own relationship computation.
                // The full C++ implementation calls m_player->getRelationship(other->getTeam()).
                // Since we don't have Player::getRelationship here, we check if
                // the other object is enemy/ally/neutral to us via kind-of heuristics.
                let rel = compute_player_affiliation(self.player_id, &other_guard);

                let matches = match rel {
                    Relationship::Enemy => self.affiliation & AFFILIATION_ALLOW_ENEMIES != 0,
                    Relationship::Neutral => self.affiliation & AFFILIATION_ALLOW_NEUTRAL != 0,
                    Relationship::Ally | Relationship::Allies | Relationship::Friend => {
                        self.affiliation & AFFILIATION_ALLOW_ALLIES != 0
                    }
                };

                if matches {
                    return self.match_flag;
                }

                return !self.match_flag;
            }
        }

        !self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterPlayerAffiliation"
    }
}

/// Compute the relationship of a player to an object.
/// Approximates C++ Player::getRelationship(other->getTeam()).
fn compute_player_affiliation(player_id: PlayerId, obj: &crate::object::Object) -> Relationship {
    use crate::object::Object;
    use std::sync::RwLock;

    let obj_player_id = obj.get_player_id().unwrap_or(PlayerId::NEUTRAL);

    // Same player = friend
    if obj_player_id == player_id {
        return Relationship::Friend;
    }

    // Neutral player
    if obj_player_id == PlayerId::NEUTRAL {
        return Relationship::Neutral;
    }

    // Use relationship_to if we can find a player-owned object
    // For now, default to Neutral; the team/relationship system
    // will be properly connected when PlayerList is fully ported.
    Relationship::Neutral
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

impl super::partition_manager::PartitionFilter for PartitionFilterGarrisonable {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let garrisonable =
                    guard.is_kind_of(KindOf::Structure) && !guard.is_kind_of(KindOf::NoGarrison);
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
    _command_source: CommandSourceType,
}

impl PartitionFilterGarrisonableByPlayer {
    pub fn new(player_id: PlayerId, match_flag: bool, command_source: CommandSourceType) -> Self {
        Self {
            player_id,
            match_flag,
            _command_source: command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterGarrisonableByPlayer {
    fn allow(&self, obj: &dyn GameObject) -> bool {
        // ActionManager integration not yet fully ported.
        // Approximate: check if the building is garrisonable and the relationship allows it.
        if let Some(handle) = obj.as_object_handle() {
            if let Ok(guard) = handle.read() {
                let garrisonable =
                    guard.is_kind_of(KindOf::Structure) && !guard.is_kind_of(KindOf::NoGarrison);
                return garrisonable == self.match_flag;
            }
        }
        !self.match_flag
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
    _source_id: ObjectId,
    _command_button_id: u32,
    match_flag: bool,
    _command_source: CommandSourceType,
}

impl PartitionFilterValidCommandButtonTarget {
    pub fn new(
        source_id: ObjectId,
        command_button_id: u32,
        match_flag: bool,
        command_source: CommandSourceType,
    ) -> Self {
        Self {
            _source_id: source_id,
            _command_button_id: command_button_id,
            match_flag,
            _command_source: command_source,
        }
    }
}

impl super::partition_manager::PartitionFilter for PartitionFilterValidCommandButtonTarget {
    fn allow(&self, _obj: &dyn GameObject) -> bool {
        // CommandButton validation requires UI/CommandButton system not yet ported.
        // Placeholder: accept all objects.
        self.match_flag
    }

    fn debug_name(&self) -> &'static str {
        "PartitionFilterValidCommandButtonTarget"
    }
}
