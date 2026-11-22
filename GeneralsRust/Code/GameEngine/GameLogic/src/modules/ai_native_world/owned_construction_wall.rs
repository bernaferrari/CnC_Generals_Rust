//! Receiving-world half of Player.cpp's completed construction footprint pass.
use super::*;
use crate::damage::{DamageInfo, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::path::pathfind_layer_classify::WallFootprint;

impl BorrowedAiWorld<'_> {
    /// Player.cpp:1644–1645 removes and re-adds a completed structure on the
    /// receiving Pathfinder. Removing a live wall piece also splats wall-layer
    /// occupants before wall reclassification, as AIPathfind.cpp:4107–4153.
    pub(crate) fn refresh_native_construction_footprint(
        &mut self,
        structure: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let (id, has_wall_removal, wall_footprint) = {
            let object = structure
                .try_read()
                .map_err(|_| "construction structure unavailable")?;
            let actual = self
                .object(object.get_id())
                .ok_or("construction structure absent from receiving factory")?;
            if !Arc::ptr_eq(&actual, structure) {
                return Err("construction structure belongs to another factory".into());
            }
            (
                object.get_id(),
                object.is_kind_of(KindOf::WalkOnTopOfWall)
                    && crate::ai::pathfind_complete::PathfindingSystem::
                        removal_reaches_wall_side_effects(&object),
                WallFootprint::from_object(&object),
            )
        };

        let has_wall_layer = has_wall_removal
            && self
                .services
                .pathfinder
                .pathfinding_system_mut()
                .wall_layer_is_allocated();
        if has_wall_layer {
            let object_ids = self.update_scheduler.get_all_object_ids().to_vec();
            let mut fall_candidates = Vec::new();
            for occupant_id in object_ids {
                let Some(occupant) = self.object(occupant_id) else {
                    continue;
                };
                let fell_from_wall = {
                    let object = occupant
                        .try_read()
                        .map_err(|_| "wall occupant unavailable during construction")?;
                    object.get_layer() == crate::common::PathfindLayerEnum::Wall
                        && wall_footprint.contains(object.get_position())
                };
                if !fell_from_wall {
                    continue;
                }
                let damage = DamageInfo::with_simple(
                    HUGE_DAMAGE_AMOUNT,
                    occupant_id,
                    DamageType::Falling,
                    DeathType::Splatted,
                );
                self.validate_damage_owned(&occupant, &damage)?;
                fall_candidates.push((occupant_id, occupant));
            }

            // C++ removes the wall-list entry before applying already-admitted
            // fall damage, then reclassifies after all synchronous callbacks.
            self.services
                .pathfinder
                .pathfinding_system_mut()
                .remove_wall_piece(id);
            for (occupant_id, occupant) in fall_candidates {
                let still_fell = {
                    let object = occupant
                        .try_read()
                        .map_err(|_| "wall occupant unavailable during construction")?;
                    object.get_layer() == crate::common::PathfindLayerEnum::Wall
                        && wall_footprint.contains(object.get_position())
                };
                if !still_fell {
                    continue;
                }
                let mut damage = DamageInfo::with_simple(
                    HUGE_DAMAGE_AMOUNT,
                    occupant_id,
                    DamageType::Falling,
                    DeathType::Splatted,
                );
                self.attempt_damage_owned(&occupant, &mut damage)?;
            }

            // Recompute from IDs still held by this exact Pathfinder and
            // geometry resolved through this world's ObjectFactory.
            let wall_ids = self
                .services
                .pathfinder
                .pathfinding_system_mut()
                .wall_piece_ids()
                .to_vec();
            let mut footprints = Vec::with_capacity(wall_ids.len());
            for wall_id in wall_ids {
                let Some(wall) = self.object(wall_id) else {
                    continue;
                };
                let wall = wall
                    .try_read()
                    .map_err(|_| "remaining wall piece unavailable")?;
                footprints.push(WallFootprint::from_object(&wall));
            }
            self.services
                .pathfinder
                .pathfinding_system_mut()
                .reclassify_wall_cells_from_footprints(&footprints);
        }

        // The C++ remove/add pair classifies the ordinary structure obstacle
        // after wall occupants have fallen and wall cells have been rebuilt.
        let object = structure
            .try_read()
            .map_err(|_| "construction structure unavailable for footprint")?;
        self.services
            .pathfinder
            .refresh_completed_object_footprint(&object)
    }
}
