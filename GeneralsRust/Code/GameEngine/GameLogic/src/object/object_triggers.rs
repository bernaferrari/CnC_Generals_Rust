//! Split-out inherent `triggers, position, transform, pathfind layer` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// Get unit direction vector in 2D (x, y) based on the object's facing angle.
    /// Returns (cos(angle), sin(angle)) representing the forward direction.
    /// C++ Reference: Object::getUnitDirectionVector2D
    pub fn get_unit_direction_vector_2d(&self) -> (f32, f32) {
        let angle = self.geometry_info.angle;
        (angle.cos(), angle.sin())
    }

    /// Convert bone local position to world position/transform
    /// Takes optional bone position and optional transform matrix
    /// Returns a Matrix3D representing the world transform
    /// C++ Reference: Object.cpp - bone coordinate transformation
    pub fn convert_bone_pos_to_world_pos(
        &self,
        bone_pos: Option<&Coord3D>,
        transform: Option<&Matrix3D>,
    ) -> Matrix3D {
        let object_transform = self.get_transform_matrix();
        let world_transform = if let Some(local) = transform {
            object_transform * *local
        } else {
            object_transform
        };

        if let Some(pos) = bone_pos {
            world_transform * Matrix3D::from_translation(*pos)
        } else {
            world_transform
        }
    }

    /// Set the object's transform matrix
    /// C++ Reference: Object.cpp - transform matrix setter
    pub fn set_transform_matrix(&mut self, matrix: &Matrix3D) {
        let (_, rotation, translation) = matrix.to_scale_rotation_translation();
        self.geometry_info.position = translation;
        self.geometry_info.angle = rotation.to_euler(EulerRot::XYZ).2;

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable) = drawable.write() {
                drawable.set_transform(*matrix);
            }
        }
    }

    pub fn get_multi_logical_bone_position(
        &self,
        bone_prefix: &str,
        max_bones: usize,
    ) -> Vec<Coord3D> {
        let Some(drawable) = &self.drawable else {
            return Vec::new();
        };

        let Ok(draw_guard) = drawable.read() else {
            return Vec::new();
        };

        let positions = draw_guard.get_pristine_bone_positions(bone_prefix, 1, max_bones);
        let transforms = draw_guard.get_pristine_bone_transforms(bone_prefix, 1, max_bones);
        let count = positions.len().min(transforms.len());

        let mut world_positions = Vec::with_capacity(count);
        for i in 0..count {
            let world_transform =
                self.convert_bone_pos_to_world_pos(Some(&positions[i]), Some(&transforms[i]));
            let (_, _, translation) = world_transform.to_scale_rotation_translation();
            world_positions.push(translation);
        }

        world_positions
    }

    /// Get single logical bone position and transform (C++ Object::getSingleLogicalBonePosition).
    pub fn get_single_logical_bone_position(&self, bone_name: &str) -> (bool, Coord3D, Matrix3D) {
        let mut position = *self.get_position();
        let mut transform = self.get_transform_matrix();

        let Some(drawable) = &self.drawable else {
            return (false, position, transform);
        };

        let Ok(draw_guard) = drawable.read() else {
            return (false, position, transform);
        };

        let positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
        if positions.len() != 1 {
            return (false, position, transform);
        }

        let bone_pos = positions[0];
        let bone_transform = draw_guard
            .get_pristine_bone_transforms(bone_name, 0, 1)
            .get(0)
            .copied()
            .unwrap_or(Matrix3D::IDENTITY);

        let world_transform =
            self.convert_bone_pos_to_world_pos(Some(&bone_pos), Some(&bone_transform));
        let (_, _, translation) = world_transform.to_scale_rotation_translation();
        position = translation;
        transform = world_transform;

        (true, position, transform)
    }

    /// Get single logical bone position on turret (C++ Object::getSingleLogicalBonePositionOnTurret).
    pub fn get_single_logical_bone_position_on_turret(
        &self,
        turret: TurretType,
        bone_name: &str,
    ) -> (bool, Coord3D, Matrix3D) {
        let mut position = *self.get_position();
        let mut transform = self.get_transform_matrix();

        let Some(drawable) = &self.drawable else {
            return (false, position, transform);
        };
        let Some(ai) = self.get_ai_update_interface() else {
            return (false, position, transform);
        };

        let Ok(draw_guard) = drawable.read() else {
            return (false, position, transform);
        };

        let launch = drawable.get_projectile_launch_offset(
            crate::common::WeaponSlotType::Primary,
            1,
            turret,
        );
        let Some(launch) = launch else {
            return (false, position, transform);
        };

        let bone_positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
        if bone_positions.len() != 1 {
            return (false, position, transform);
        }
        let bone_pos = bone_positions[0];

        let (turret_rotation, _) = ai
            .lock()
            .ok()
            .and_then(|guard| guard.get_turret_rot_and_pitch(turret))
            .unwrap_or((0.0, 0.0));

        let bone_offset = Matrix3D::from_translation(bone_pos);
        let turn_adjustment = Matrix3D::from_translation(launch.turret_rot_pos)
            * Matrix3D::from_rotation_z(turret_rotation)
            * Matrix3D::from_translation(-launch.turret_rot_pos);

        let bone_logic_transform = turn_adjustment * bone_offset;
        let world_transform = self.convert_bone_pos_to_world_pos(None, Some(&bone_logic_transform));
        let (_, _, translation) = world_transform.to_scale_rotation_translation();
        position = translation;
        transform = world_transform;

        (true, position, transform)
    }

    /// Get object position
    pub fn get_position(&self) -> &Coord3D {
        &self.geometry_info.position
    }

    pub fn get_template_geometry_type(
        &self,
    ) -> Option<game_engine::system::geometry::GeometryType> {
        self.thing_template.get_template_geometry_type()
    }

    /// Get object orientation (radians)
    pub fn get_orientation(&self) -> Real {
        self.geometry_info.angle
    }

    /// Tiny fudge so "on ground" (z ~= layer height) is not treated as airborne.
    /// C++ Thing::isAboveTerrain uses `getHeightAboveTerrain() > 0.0f`.
    pub const ABOVE_TERRAIN_SLOP: Real = 0.0;

    /// C++ Thing::isAboveTerrain comparison: `z > ground height + slop`.
    #[inline]
    pub fn is_above_terrain_height(z: Real, ground_height: Real) -> bool {
        z > ground_height + Self::ABOVE_TERRAIN_SLOP
    }

    /// Get height above the terrain.
    pub fn get_height_above_terrain(&self) -> Real {
        self.geometry_info.height_above_terrain
    }

    /// Update cached height above the terrain (used by physics/locomotor).
    pub fn set_height_above_terrain(&mut self, height: Real) {
        self.geometry_info.height_above_terrain = height;
    }

    /// C++ parity: Object::calculateHeightAboveTerrain() (Object.cpp line 2751)
    pub fn calculate_height_above_terrain(&self) -> Real {
        let pos = self.get_position();
        pos.z - self.ground_height_for_above_terrain()
    }

    /// Ground/layer height used by [`Self::is_above_terrain`].
    ///
    /// C++ Object::calculateHeightAboveTerrain uses `TheTerrainLogic->getLayerHeight`.
    /// Missing terrain (unit tests) stubs ground height as 0.
    pub(super) fn ground_height_for_above_terrain(&self) -> Real {
        let pos = self.get_position();
        if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
            terrain.get_layer_height(pos.x, pos.y, self.layer)
        } else {
            0.0
        }
    }

    /// Returns true if the object is well above the ground plane. Matches the C++ helper used
    /// by crates to prevent airborne pickups.
    pub fn is_significantly_above_terrain(&self) -> bool {
        self.get_height_above_terrain() > 1.0
    }

    /// Returns true if the object is currently treated as airborne.
    pub fn is_using_airborne_locomotor(&self) -> bool {
        self.is_airborne_target()
    }

    /// Set object position
    pub fn set_position(&mut self, position: &Coord3D) -> Result<(), String> {
        self.geometry_info.position = position.clone();
        let geom = Self::collision_geometry_from_bounds(
            &self.geometry_info,
            self.get_template_geometry_type(),
        );
        let _ = crate::object::collide::collision_system::with_collision_system_mut(|system| {
            let collision_pos =
                crate::object::collide::Coord3D::new(position.x, position.y, position.z);
            let res = system.update_object_position(self.id, collision_pos);
            if res.is_err() {
                let _ = system.register_object(self.id, collision_pos, geom, None);
            }
            Ok::<(), crate::object::collide::CollisionError>(())
        });
        if !self.is_kind_of(KindOf::Projectile) && !self.is_kind_of(KindOf::Inert) {
            let area_tracker = crate::scripting::engine::get_area_tracker();
            let event_manager = crate::scripting::engine::get_event_manager();
            if let Err(err) = area_tracker.update_object_position_sync(
                self.id,
                [position.x, position.y, position.z],
                &event_manager,
            ) {
                warn!(
                    "Failed to update area tracker for object {}: {}",
                    self.id, err
                );
            }
        }

        // C++ Object.cpp lines 2542-2651: Update trigger area flags when position changes
        self.set_trigger_area_flags_for_change_in_position();

        Ok(())
    }

    /// Update trigger area flags when object position changes.
    /// C++ Reference: Object.cpp lines 2542-2651
    ///
    /// This method:
    /// - Skips projectiles and inert objects (they don't trigger areas)
    /// - Updates pathfinding position
    /// - Checks for exited/entered trigger areas
    /// - Updates integer position tracking for efficient trigger checks
    pub(super) fn set_trigger_area_flags_for_change_in_position(&mut self) {
        // projectiles cannot trigger areas. (jkmcd)
        // neither can inert objects, like the radar ping, etc. (jkmcd)
        if self.is_kind_of(KindOf::Projectile) || self.is_kind_of(KindOf::Inert) {
            return;
        }

        let pos = self.get_position();
        let new_i_pos = ICoord3D {
            x: pos.x as Int,
            y: pos.y as Int,
            z: 0, // Trigger areas compare on xy only
        };

        // C++ lines 2554-2556: Didn't move enough to change integer position
        if self.i_pos.x == new_i_pos.x && self.i_pos.y == new_i_pos.y {
            return;
        }

        // C++ Object.cpp:2580-2583 notifyTerrainObjectMoved for infantry/vehicle.
        if !self.is_kind_of(KindOf::Immobile)
            && (self.is_kind_of(KindOf::Infantry) || self.is_kind_of(KindOf::Vehicle))
        {
            if let Some(client) = crate::helpers::TheGameClient::get() {
                client.notify_terrain_object_moved(self.id);
            }
        }

        // C++ lines 2565-2568: Update pathfinder position
        if self.get_ai_update_interface().is_some() {
            // TheAI->pathfinder()->updatePos(this, getPosition()) - handled by AI system
        }

        let now = crate::helpers::TheGameLogic::get_frame();

        // C++ lines 2570-2572: Update trigger area flags if not current frame
        if self.entered_or_exited_frame != 0 && self.entered_or_exited_frame != now {
            self.update_trigger_area_flags();
        }

        // C++ lines 2574-2590: Check for exited trigger areas.
        // C++ Object.cpp:2599 uses the *old* m_iPos, not the new cell.
        let old_i_pos = self.i_pos;
        for i in 0..(self.num_trigger_areas_active as usize) {
            if self.num_trigger_areas_active as usize >= MAX_TRIGGER_AREA_INFOS {
                break;
            }
            let trigger = &self.trigger_info[i].trigger;
            if let Some(trigger_arc) = trigger {
                let inside = trigger_arc.point_in_trigger_int(&old_i_pos);
                if !inside {
                    self.trigger_info[i].is_inside = false;
                    self.trigger_info[i].exited = true;
                    self.entered_or_exited_frame = now;
                    if let Some(team) = self.get_team() {
                        if let Ok(mut team_guard) = team.write() {
                            team_guard.set_entered_exited();
                        }
                    }
                    crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(self.id);
                }
            }
        }

        // C++ line 2593: Update integer position
        self.i_pos = new_i_pos;

        // C++ lines 2595-2651: Check for newly entered trigger areas.
        // Iterate every PolygonTrigger, not only already-tracked ones.
        self.enter_untracked_polygon_triggers(now);
    }

    /// C++ Object.cpp:2615-2657 — walk `PolygonTrigger::getFirstPolygonTrigger()`.
    fn enter_untracked_polygon_triggers(&mut self, now: UnsignedInt) {
        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return;
        };
        let triggers: Vec<Arc<PolygonTrigger>> = terrain
            .get_trigger_areas()
            .get_triggers()
            .iter()
            .cloned()
            .map(Arc::new)
            .collect();
        drop(terrain);

        for trigger_arc in triggers {
            let already_tracked = (0..(self.num_trigger_areas_active as usize)).any(|i| {
                self.trigger_info[i]
                    .trigger
                    .as_ref()
                    .is_some_and(|tracked| tracked.get_id() == trigger_arc.get_id())
            });
            if already_tracked {
                continue;
            }
            if !trigger_arc.point_in_trigger_int(&self.i_pos) {
                continue;
            }
            let slot = self.num_trigger_areas_active as usize;
            if slot >= MAX_TRIGGER_AREA_INFOS {
                break;
            }
            self.trigger_info[slot].is_inside = true;
            self.trigger_info[slot].entered = true;
            self.trigger_info[slot].exited = false;
            self.trigger_info[slot].trigger = Some(trigger_arc);
            self.entered_or_exited_frame = now;
            if let Some(team) = self.get_team() {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.set_entered_exited();
                }
            }
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(self.id);
            self.num_trigger_areas_active += 1;
        }
    }

    /// Update trigger area flags, clearing entered/exited markers.
    /// C++ Reference: Object.cpp lines 2351-2365
    pub(super) fn update_trigger_area_flags(&mut self) {
        let mut j = 0;
        for i in 0..(self.num_trigger_areas_active as usize) {
            if !self.trigger_info[i].is_inside {
                continue;
            }
            self.trigger_info[j].entered = false;
            self.trigger_info[j].exited = false;
            self.trigger_info[j].is_inside = self.trigger_info[i].is_inside;
            self.trigger_info[j].trigger = self.trigger_info[i].trigger.clone();
            j += 1;
        }
        self.num_trigger_areas_active = j as u8;
    }

    /// Returns whether an object entered or exited an area.
    /// C++ Reference: Object.cpp lines 2467-2478
    pub fn did_enter_or_exit(&self) -> bool {
        if self.is_kind_of(KindOf::Inert) {
            return false;
        }
        // note that this needs to return true if we
        // entered or exited on the current frame OR
        // the previous frame... since the current execution
        // order is ScriptEngine, then ObjectUpdates,
        // enter/exits detected in ObjectUpdate on frame N
        // won't be noticed by the ScriptEngine till frame N+1.
        let now = crate::helpers::TheGameLogic::get_frame();
        self.entered_or_exited_frame == now || self.entered_or_exited_frame == now - 1
    }

    /// Returns whether an object entered a specific trigger area.
    /// C++ Reference: Object.cpp lines 2483-2496
    pub fn did_enter(&self, trigger: &PolygonTrigger) -> bool {
        if !self.did_enter_or_exit() {
            return false;
        }

        for i in 0..(self.num_trigger_areas_active as usize) {
            if self.trigger_info[i].entered {
                if let Some(t) = &self.trigger_info[i].trigger {
                    if t.same_list_node(trigger) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns whether an object exited a specific trigger area.
    /// C++ Reference: Object.cpp lines 2501-2514
    pub fn did_exit(&self, trigger: &PolygonTrigger) -> bool {
        if !self.did_enter_or_exit() {
            return false;
        }

        for i in 0..(self.num_trigger_areas_active as usize) {
            if self.trigger_info[i].exited {
                if let Some(t) = &self.trigger_info[i].trigger {
                    if t.same_list_node(trigger) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns whether an object is inside a specific trigger area.
    /// C++ Reference: Object.cpp lines 2519-2529
    pub fn is_inside_trigger(&self, trigger: &PolygonTrigger) -> bool {
        for i in 0..(self.num_trigger_areas_active as usize) {
            if self.trigger_info[i].is_inside {
                if let Some(t) = &self.trigger_info[i].trigger {
                    if t.same_list_node(trigger) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn collision_geometry_from_bounds(
        info: &crate::common::GeometryInfo,
        template_type: Option<game_engine::system::geometry::GeometryType>,
    ) -> crate::object::collide::collision_geometry::GeometryInfo {
        let dx = info.bounds.max.x - info.bounds.min.x;
        let dy = info.bounds.max.y - info.bounds.min.y;
        let dz = info.bounds.max.z - info.bounds.min.z;
        let radius = (dx.max(dy) * 0.5).max(0.01);
        let height = dz.max(0.01);
        let is_small = radius < 1.0;
        match template_type {
            Some(game_engine::system::geometry::GeometryType::Sphere) => {
                crate::object::collide::collision_geometry::GeometryInfo::new_sphere(
                    radius, is_small,
                )
            }
            Some(game_engine::system::geometry::GeometryType::Box) => {
                crate::object::collide::collision_geometry::GeometryInfo::new_box(
                    dx.max(0.01),
                    dy.max(0.01),
                    is_small,
                )
            }
            Some(game_engine::system::geometry::GeometryType::Cylinder) => {
                crate::object::collide::collision_geometry::GeometryInfo::new_cylinder(
                    radius, height, is_small,
                )
            }
            None => {
                if height <= radius * 0.5 {
                    crate::object::collide::collision_geometry::GeometryInfo::new_sphere(
                        radius, is_small,
                    )
                } else {
                    crate::object::collide::collision_geometry::GeometryInfo::new_cylinder(
                        radius, height, is_small,
                    )
                }
            }
        }
    }

    /// Get carrier deck height offset (used for deck-taxiing logic).
    pub fn get_carrier_deck_height(&self) -> Real {
        self.carrier_deck_height
    }

    /// Set carrier deck height offset (used for deck-taxiing logic).
    pub fn set_carrier_deck_height(&mut self, height: Real) {
        self.carrier_deck_height = height;
    }

    /// Set object orientation (stored on geometry info; rendering updates occur elsewhere).
    pub fn set_orientation(&mut self, angle: Real) -> Result<(), String> {
        self.geometry_info.angle = angle;
        Ok(())
    }

    /// Returns true if the object is currently flagged as outside the playable area.
    pub fn is_off_map(&self) -> bool {
        let Some(terrain) = crate::helpers::TheTerrainLogic::get() else {
            return false;
        };
        let extent = terrain.get_maximum_pathfind_extent();
        let pos = self.get_position();
        pos.x < extent.lo.x || pos.x > extent.hi.x || pos.y < extent.lo.y || pos.y > extent.hi.y
    }

    //=========================================================================
    // Object API helpers used by behaviors
    //=========================================================================

    /// Check if object is above terrain (not on ground).
    ///
    /// C++ Thing.h: `isAboveTerrain() { return getHeightAboveTerrain() > 0.0f; }`
    /// Object overrides height via layer (bridges): `z > ground/layer height + slop`.
    pub fn is_above_terrain(&self) -> bool {
        Self::is_above_terrain_height(
            self.get_position().z,
            self.ground_height_for_above_terrain(),
        )
    }

    /// Get the transform matrix for this object
    /// C++ Reference: Object.cpp - Transform matrix accessor
    ///
    /// # Returns
    /// The transformation matrix for this object (position, rotation, scale)
    pub fn get_transform_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.geometry_info.position)
            * Mat4::from_rotation_z(self.geometry_info.angle)
    }

    //=========================================================================
    // Object API helpers used by modules
    //=========================================================================

    /// C++ `Object::getLayer` (Object.h:380).
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// C++ `Object::setLayer` (Object.cpp:2699-2714).
    ///
    /// OCL `PreserveLayer` / `ON_GROUND_ALIGNED` call this so debris stays on
    /// the source object's pathfind layer (bridge/wall) instead of dropping to
    /// ground. Pathfinder occupancy is refreshed when TheAI is available.
    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        if layer == self.layer {
            return;
        }
        // C++: TheAI->pathfinder()->removePos(this);
        self.sync_pathfinder_pos(true);
        self.layer = layer;
        // C++: TheAI->pathfinder()->updatePos(this, getPosition());
        self.sync_pathfinder_pos(false);
    }

    /// Best-effort C++ Pathfinder::removePos / updatePos around a layer change.
    /// No-ops when AI/pathfinder is not constructed (unit tests, early boot).
    pub(super) fn sync_pathfinder_pos(&self, remove_only: bool) {
        let ai_store = crate::ai::the_ai();let Ok(ai_guard) = ai_store.read() else {
            return;
        };
        let Some(pathfinder) = ai_guard.pathfinder() else {
            return;
        };
        let Ok(pf) = pathfinder.write() else {
            return;
        };
        let cell = crate::ai::pathfind_astar::GridCoord::from_world(self.get_position());
        let astar_layer = {
            let v = self.layer as u32;
            if (2..=14).contains(&v) {
                crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(v)
            } else if self.layer == PathfindLayerEnum::Wall {
                crate::ai::pathfind_astar::PathfindLayerEnum::Wall
            } else {
                crate::ai::pathfind_astar::PathfindLayerEnum::Ground
            }
        };
        if remove_only {
            pf.remove_pos_cells(self.get_id(), 0, true, astar_layer);
        } else {
            let interacts = crate::terrain::get_terrain_logic()
                .read()
                .ok()
                .map(|t| {
                    t.object_interacts_with_bridge_end(
                        self,
                        crate::path::PathfindLayerEnum::from_u32(self.layer as u32),
                    )
                })
                .unwrap_or(false);
            pf.update_pos_cells(cell, self.get_id(), astar_layer, 0, true, interacts);
        }
    }

    pub fn get_destination_layer(&self) -> PathfindLayerEnum {
        self.destination_layer
    }

    pub fn set_destination_layer(&mut self, layer: PathfindLayerEnum) {
        self.destination_layer = layer;
    }
}

#[cfg(test)]
mod trigger_identity_tests {
    use super::*;
    use crate::common::{AsciiString, Coord3D, DefaultThingTemplate, ICoord3D, KindOf};
    use crate::polygon_trigger::PolygonTrigger;
    use crate::scripting::engine::{get_area_tracker, get_event_manager};
    use crate::scripting::events::TriggerArea;
    use crate::system::game_logic::get_game_logic;
    use crate::terrain::get_terrain_logic;
    use std::sync::Arc;

    fn square(id: i32, name: &str) -> PolygonTrigger {
        PolygonTrigger::new(
            id,
            AsciiString::from(name),
            vec![
                ICoord3D::new(0, 0, 0),
                ICoord3D::new(10, 0, 0),
                ICoord3D::new(10, 10, 0),
                ICoord3D::new(0, 10, 0),
            ],
        )
    }

    #[test]
    fn is_inside_trigger_true_when_unit_inside_named_area() {
        // C++ Object::isInside (Object.cpp:2519-2529) compares the stored
        // PolygonTrigger* to the same list node. Rust stores cloned Arcs, so
        // identity is the trigger id (list-node handle), not Arc::ptr_eq.
        let _lock = crate::test_sync::lock();
        let area_name = "IdentityInsideArea";
        let trigger = square(4242, area_name);
        get_terrain_logic()
            .write()
            .expect("terrain")
            .add_trigger_area(trigger.clone());

        get_game_logic()
            .lock()
            .expect("logic")
            .set_current_frame(10);

        let mut obj = Object::new_test(0x00B0_1B00, 100.0);
        obj.set_position(&Coord3D::new(5.0, 5.0, 0.0))
            .expect("move inside");

        let query = square(4242, area_name);
        assert!(
            obj.is_inside_trigger(&query),
            "fresh PolygonTrigger with the same id must match the stored handle"
        );
        assert!(obj.did_enter(&query));
    }

    #[test]
    fn projectile_set_position_does_not_record_area_enter() {
        // C++ Object::setTriggerAreaFlagsForChangeInPosition (Object.cpp:2565-2568)
        // early-returns for KINDOF_PROJECTILE / KINDOF_INERT.
        let _lock = crate::test_sync::lock();
        let area_name = "ProjectileIgnoreArea";
        let object_id = 0x0090_6C01;
        let tracker = get_area_tracker();
        let _ = tracker.unregister_area(area_name);
        tracker
            .register_area(TriggerArea::new_rectangular(
                area_name.to_string(),
                [0.0, 0.0, 0.0],
                0.0,
                0.0,
                20.0,
                20.0,
            ))
            .expect("register");

        let mut template = DefaultThingTemplate::new("TestProjectile".to_string());
        template.add_kind_of(KindOf::Projectile);
        let mut obj = Object::new_test_from_template(object_id, 10.0, Arc::new(template));
        obj.set_position(&Coord3D::new(5.0, 5.0, 0.0))
            .expect("move projectile");

        assert!(
            !tracker
                .is_object_in_area(object_id, area_name)
                .unwrap_or(true),
            "projectiles must not fire AreaTracker enter events"
        );
        let _ = tracker.unregister_area(area_name);
        let _ = get_event_manager();
    }
}
