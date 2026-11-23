//! Split-out inherent `triggers, position, transform, pathfind layer` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

// Matrix3D.cpp:226 / WWMath::Atan2 promotes both float components to the
// double libm call and casts its result back to float. atan2f can differ by
// one ULP even at the signed -PI boundary.
pub(super) fn cpp_matrix_z_rotation(matrix: &Matrix3D) -> Real {
    (matrix.x_axis.y as f64).atan2(matrix.x_axis.x as f64) as Real
}

// Common/System/GameCommon.cpp27-42: -PI belongs to the +PI boundary.
// Callers admitting saved input validate finite/canonical angles first.
pub(super) fn normalize_orientation_angle(mut angle: Real) -> Real {
    let pi = std::f32::consts::PI;
    while angle > pi {
        angle -= 2.0 * pi;
    }
    while angle <= -pi {
        angle += 2.0 * pi;
    }
    angle
}

#[cfg(test)]
mod owned_orientation_tests;

/// Object.cpp:1683-1697 uses a strict per-axis threshold.
fn is_position_different(old: Coord3D, next: Coord3D) -> bool {
    (old.x - next.x).abs() > 0.01
        || (old.y - next.y).abs() > 0.01
        || (old.z - next.z).abs() > 0.01
}

pub(crate) struct OwnedObjectTransform {
    pub(crate) position_changed: bool,
    pub(crate) pose_changed: bool,
}

/// Active map extent of the actor's world, as in Object.cpp:1764–1771.
/// Detached compatibility actors may still use the installed services.
fn receiving_terrain_extent(object: &Object) -> Option<crate::common::Region3D> {
    let services = object
        .owning_world_services()
        .or_else(crate::system::engine_stores::try_active_services)?;
    let terrain = services.terrain().read().ok()?;
    Some(active_compatibility_extent(&terrain))
}

fn active_compatibility_extent(terrain: &crate::terrain::TerrainLogic) -> crate::common::Region3D {
    let extent = terrain.get_extent();
    if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
        return extent;
    }
    let extent = terrain.get_maximum_pathfind_extent();
    if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
        return extent;
    }
    crate::common::Region3D::new(
        crate::common::Coord3D::new(-1.0, -1.0, 0.0),
        crate::common::Coord3D::new(50001.0, 50001.0, 0.0),
    )
}

/// Resolve the active extent before a compatibility pose mutation. Unlike the
/// older best-effort refresh, this path reports an owned terrain borrow failure
/// so the caller can leave the pose and OFF_MAP bit untouched.
fn checked_receiving_terrain_extent(
    object: &Object,
) -> Result<Option<crate::common::Region3D>, String> {
    let Some(services) = object
        .owning_world_services()
        .or_else(crate::system::engine_stores::try_active_services)
    else {
        return Ok(None);
    };
    let terrain = services
        .terrain()
        .try_read()
        .map_err(|_| "Object position terrain is already borrowed")?;
    // Keep the existing maximum-boundary/unloaded fallback when no active map
    // extent is installed, but never use it in place of a valid active extent.
    Ok(Some(active_compatibility_extent(&terrain)))
}

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
        // Thing.cpp317-335 computes matrix and point independently. The bone
        // matrix already contains its translation; applying the point again
        // doubles that offset (and can rotate it a second time).
        if let Some(local) = transform {
            object_transform * *local
        } else if let Some(pos) = bone_pos {
            let mut world_transform = object_transform;
            world_transform.w_axis = object_transform.transform_point3(*pos).extend(1.0);
            world_transform
        } else {
            object_transform
        }
    }

    /// Set the object's transform matrix
    /// C++ Reference: Object.cpp - transform matrix setter
    pub fn set_transform_matrix(&mut self, matrix: &Matrix3D) {
        self.set_transform_matrix_with_trigger_context(matrix, None, &mut |id| {
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(id);
        });
    }

    /// The object load caller already borrows its driving GameLogic. Trigger
    /// notifications remain synchronous, without reacquiring that same owner.
    pub(super) fn set_transform_matrix_with_trigger_context(
        &mut self,
        matrix: &Matrix3D,
        frame: Option<UnsignedInt>,
        changed: &mut dyn FnMut(ObjectID),
    ) {
        self.set_transform_matrix_with_pose_and_extent(
            matrix,
            None,
            frame,
            changed,
            &mut Self::calculate_height_above_terrain,
            receiving_terrain_extent,
        );
    }

    fn set_transform_matrix_with_pose_and_extent(
        &mut self,
        matrix: &Matrix3D,
        cached_pose: Option<(Coord3D, Real)>,
        frame: Option<UnsignedInt>,
        changed: &mut dyn FnMut(ObjectID),
        height: &mut dyn FnMut(&Object) -> Real,
        extent_for_refresh: impl FnOnce(&Object) -> Option<crate::common::Region3D>,
    ) {
        let (pos_diff, ang_diff) = self.apply_transform_local(matrix, cached_pose, height);
        let translation = self.geometry_info.position;
        let new_angle = self.geometry_info.angle;
        if pos_diff || ang_diff {
            self.react_contain_to_transform();
        }
        if pos_diff || ang_diff {
            let geom = Self::collision_geometry_from_bounds(
                &self.geometry_info,
                self.get_template_geometry_type(),
            );
            let _ = crate::object::collide::collision_system::with_collision_system_mut(|system| {
                let collision_pos = crate::object::collide::Coord3D::new(
                    translation.x,
                    translation.y,
                    translation.z,
                );
                let res = system.update_object_pose(self.id, collision_pos, new_angle);
                if res.is_err() {
                    let _ = system.register_object(self.id, collision_pos, geom, None);
                }
                Ok::<(), crate::object::collide::CollisionError>(())
            });
        }
        if pos_diff {
            self.set_trigger_area_flags_with_context(frame, changed);
            // C++ reactToTransformChange refreshes OFF_MAP from active bounds.
            if let Some(extent) = extent_for_refresh(self) {
                self.refresh_off_map_status(&extent);
            }
        }
    }

    /// The identical local transform phase used by both service adapters.
    fn apply_transform_local(
        &mut self,
        matrix: &Matrix3D,
        cached_pose: Option<(Coord3D, Real)>,
        height: &mut dyn FnMut(&Object) -> Real,
    ) -> (bool, bool) {
        // Thing.cpp:226-241 copies the rows directly. Translation does not
        // require decomposition: finite shear/singular bases are valid rows.
        let (translation, new_angle) = cached_pose
            .unwrap_or_else(|| (matrix.w_axis.truncate(), cpp_matrix_z_rotation(matrix)));
        let old_pos = self.geometry_info.position;
        let old_angle = self.geometry_info.angle;
        self.transform_matrix = *matrix;
        self.geometry_info.position = translation;
        self.geometry_info.angle = new_angle;
        self.geometry_info.height_above_terrain = height(self);

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable) = drawable.write() {
                drawable.set_transform(*matrix);
            }
        }
        let pos_diff = is_position_different(old_pos, translation);
        let ang_diff = (old_angle - new_angle).abs() > 0.01;
        (pos_diff, ang_diff)
    }

    /// Explicit native transform for the admitted leaf passenger family. A
    /// passenger that itself contains objects requires its own virtual callback
    /// capability and is rejected before any field changes.
    pub(crate) fn set_transform_matrix_with_services(
        &mut self,
        matrix: &Matrix3D,
        terrain: &crate::terrain::TerrainLogic,
        collision: &mut crate::object::collide::collision_system::CollisionSystem,
        frame: u32,
        changed: &mut dyn FnMut(ObjectID),
        notify_terrain_moved: &mut dyn FnMut(&Object),
    ) -> Result<(), String> {
        if self.contain.is_some() {
            return Err("leaf transform does not admit nested containment callbacks".into());
        }
        self.apply_transform_with_services(
            matrix,
            terrain,
            collision,
            frame,
            changed,
            notify_terrain_moved,
            false,
        )
    }

    /// Xfer applies the transform before module payloads/postload, so an
    /// actual fresh Open/Transport controller has no riders yet. Its real
    /// empty-roster callback still resets the original fire-point cache.
    pub(super) fn set_snapshot_transform_with_services(
        &mut self,
        matrix: &Matrix3D,
        terrain: &crate::terrain::TerrainLogic,
        collision: &mut crate::object::collide::collision_system::CollisionSystem,
        frame: u32,
        changed: &mut dyn FnMut(ObjectID),
        notify_terrain_moved: &mut dyn FnMut(&Object),
    ) -> Result<(), String> {
        self.apply_transform_with_services(
            matrix,
            terrain,
            collision,
            frame,
            changed,
            notify_terrain_moved,
            true,
        )
    }

    /// Inert factory construction: install pose without admitting effects.
    pub(super) fn set_prepared_position(&mut self, position: &Coord3D) {
        self.transform_matrix.w_axis = position.extend(1.0);
        self.geometry_info.position = *position;
    }

    fn apply_transform_with_services(
        &mut self,
        matrix: &Matrix3D,
        terrain: &crate::terrain::TerrainLogic,
        collision: &mut crate::object::collide::collision_system::CollisionSystem,
        frame: u32,
        changed: &mut dyn FnMut(ObjectID),
        notify_terrain_moved: &mut dyn FnMut(&Object),
        snapshot: bool,
    ) -> Result<(), String> {
        // Validate callbacks before changing pose. A populated owner needs
        // supplied rider transform services. An unchanged saved pose does not
        // execute this callback and therefore does not need that capability.
        let translation = matrix.w_axis.truncate();
        let angle = normalize_orientation_angle(cpp_matrix_z_rotation(matrix));
        let moved = (self.geometry_info.position - translation)
            .abs()
            .max_element()
            > 0.01
            || (normalize_orientation_angle(self.geometry_info.angle) - angle).abs() > 0.01;
        let contain = self.contain.clone();
        let mut empty_contain = if snapshot && moved {
            match contain.as_ref() {
                Some(contain) => {
                    let controller = contain
                        .lock()
                        .map_err(|_| "Object transform contain poisoned")?;
                    if controller.get_contained_count() != 0 || self.drawable.is_some() {
                        return Err(
                            "snapshot transform requires an empty headless container".into()
                        );
                    }
                    Some(controller)
                }
                None => None,
            }
        } else {
            None
        };
        let (pos_diff, ang_diff) = self.apply_transform_local(matrix, None, &mut |object| {
            let p = object.get_position();
            p.z - terrain.get_layer_height(
                p.x,
                p.y,
                crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32),
                None,
                false,
            )
        });
        if pos_diff || ang_diff {
            if let Some(contain) = empty_contain.as_mut() {
                contain.redeploy_riders_at(&self.geometry_info.position, &[]);
            }
        }
        drop(empty_contain);
        self.dirty_partition_for_owned_pose(pos_diff || ang_diff, collision)?;
        self.finish_owned_transform_with_services(
            OwnedObjectTransform {
                position_changed: pos_diff,
                pose_changed: pos_diff || ang_diff,
            },
            terrain,
            frame,
            changed,
            notify_terrain_moved,
        )?;
        Ok(())
    }

    /// Original local pose/Drawable and partition phase. The driving instance
    /// releases the Object loan before the synchronous containment callback.
    pub(crate) fn begin_owned_transform_with_services(
        &mut self,
        matrix: &Matrix3D,
        terrain: &crate::terrain::TerrainLogic,
        collision: &mut crate::object::collide::collision_system::CollisionSystem,
    ) -> Result<OwnedObjectTransform, String> {
        if !matrix.is_finite() {
            return Err("owned transform is nonfinite".into());
        }
        let (position_changed, angle_changed) =
            self.apply_transform_local(matrix, None, &mut |object| {
                let position = object.get_position();
                position.z
                    - terrain.get_layer_height(
                        position.x,
                        position.y,
                        crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32),
                        None,
                        false,
                    )
            });
        let pose_changed = position_changed || angle_changed;
        self.dirty_partition_for_owned_pose(pose_changed, collision)?;
        Ok(OwnedObjectTransform {
            position_changed,
            pose_changed,
        })
    }

    fn dirty_partition_for_owned_pose(
        &mut self,
        pose_changed: bool,
        collision: &mut crate::object::collide::collision_system::CollisionSystem,
    ) -> Result<(), String> {
        if pose_changed && self.partition_data.is_some() {
            let p = self.geometry_info.position;
            let position = crate::object::collide::Coord3D::new(p.x, p.y, p.z);
            if collision
                .update_object_pose(self.id, position, self.geometry_info.angle)
                .is_err()
            {
                let geometry = Self::collision_geometry_from_bounds(
                    &self.geometry_info,
                    self.get_template_geometry_type(),
                );
                // Same initial-placement fallback as the compatibility adapter.
                collision
                    .register_object(self.id, position, geometry, None)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub(crate) fn finish_owned_transform_with_services(
        &mut self,
        observation: OwnedObjectTransform,
        terrain: &crate::terrain::TerrainLogic,
        frame: u32,
        changed: &mut dyn FnMut(ObjectID),
        notify_terrain_moved: &mut dyn FnMut(&Object),
    ) -> Result<(), String> {
        let position_changed = observation.position_changed;
        if position_changed {
            let triggers = terrain
                .get_trigger_areas()
                .get_triggers()
                .iter()
                .cloned()
                .map(Arc::new)
                .collect::<Vec<_>>();
            self.set_trigger_area_flags_with_observations(
                Some(frame),
                changed,
                notify_terrain_moved,
                Some(triggers),
            );
            let extent = terrain.get_extent();
            self.refresh_off_map_status(&extent);
        }
        Ok(())
    }

    pub(crate) fn containment_turret_firepoint(&self, index: usize) -> Result<Matrix3D, String> {
        let yaw = if self.cached_main_turret_valid {
            self.cached_main_turret_yaw
        } else {
            let ai = self
                .get_ai_update_interface()
                .ok_or("turret containment AI absent")?;
            let ai = ai
                .try_lock()
                .map_err(|_| "turret containment AI is already borrowed")?;
            ai.get_turret_angle(crate::common::TurretType::Primary)
        };
        // Literal OpenContain.cpp1261 naming, including its zero-prefix branch.
        let name = if index < 10 {
            format!("FIREPOINT0{}", index + 1)
        } else {
            format!("FIREPOINT{}", index + 1)
        };
        Ok(self
            .turret_firepoint_matrix_at_yaw(&name, yaw)
            .unwrap_or_else(|| self.get_transform_matrix()))
    }

    pub(crate) fn containment_firepoint_matrices(
        &self,
        turret: bool,
    ) -> Result<Vec<Matrix3D>, String> {
        if !turret {
            return Ok(self.get_multi_logical_bone_matrix("FIREPOINT", 32));
        }
        let yaw = if self.cached_main_turret_valid {
            self.cached_main_turret_yaw
        } else {
            let ai = self
                .get_ai_update_interface()
                .ok_or("turret containment AI absent")?;
            let ai = ai
                .try_lock()
                .map_err(|_| "turret containment AI is already borrowed")?;
            ai.get_turret_angle(crate::common::TurretType::Primary)
        };
        let mut bones = Vec::new();
        for index in 1..=32 {
            let name = format!("FIREPOINT{index:02}");
            let Some(matrix) = self.turret_firepoint_matrix_at_yaw(&name, yaw) else {
                break;
            };
            bones.push(matrix);
        }
        Ok(bones)
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
            world_positions.push(self.get_transform_matrix().transform_point3(positions[i]));
        }

        world_positions
    }

    pub fn get_multi_logical_bone_matrix(
        &self,
        bone_prefix: &str,
        max_bones: usize,
    ) -> Vec<Matrix3D> {
        let Some(drawable) = &self.drawable else {
            return Vec::new();
        };
        let Ok(draw_guard) = drawable.read() else {
            return Vec::new();
        };
        let positions = draw_guard.get_pristine_bone_positions(bone_prefix, 1, max_bones);
        let transforms = draw_guard.get_pristine_bone_transforms(bone_prefix, 1, max_bones);
        let count = positions.len().min(transforms.len());
        let mut world = Vec::with_capacity(count);
        for i in 0..count {
            world.push(
                self.convert_bone_pos_to_world_pos(Some(&positions[i]), Some(&transforms[i])),
            );
        }
        world
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
        position = self.get_transform_matrix().transform_point3(bone_pos);
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

        let launch =
            draw_guard.projectile_launch_offset(crate::common::WeaponSlotType::Primary, 1, turret);
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

    fn turret_firepoint_matrix_at_yaw(
        &self,
        bone_name: &str,
        turret_rotation: f32,
    ) -> Option<Matrix3D> {
        let drawable = self.drawable.as_ref()?;
        let draw_guard = drawable.read().ok()?;
        let launch = draw_guard.projectile_launch_offset(
            crate::common::WeaponSlotType::Primary,
            1,
            TurretType::Primary,
        )?;
        let bone_positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
        if bone_positions.len() != 1 {
            return None;
        }
        let bone_offset = Matrix3D::from_translation(bone_positions[0]);
        let turn_adjustment = Matrix3D::from_translation(launch.turret_rot_pos)
            * Matrix3D::from_rotation_z(turret_rotation)
            * Matrix3D::from_translation(-launch.turret_rot_pos);
        let bone_logic_transform = turn_adjustment * bone_offset;
        Some(self.convert_bone_pos_to_world_pos(None, Some(&bone_logic_transform)))
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
        crate::object::die::eject_pilot_die::is_significantly_above_terrain_height(
            self.get_height_above_terrain(),
            crate::object::die::eject_pilot_die::current_gravity(),
        )
    }

    /// Returns true if the object is currently treated as airborne.
    /// C++ Object.cpp:3336 — current AI locomotor legal AIR surfaces, never
    /// the AirborneTarget altitude/status bit.
    pub fn is_using_airborne_locomotor(&self) -> bool {
        self.get_ai_update_interface()
            .and_then(|ai| {
                ai.lock().ok().map(|guard| {
                    crate::object::contain::transport_contain::selected_locomotor_is_airborne(
                        &*guard,
                    )
                })
            })
            .unwrap_or(false)
    }

    /// Set object position
    pub fn set_position(&mut self, position: &Coord3D) -> Result<(), String> {
        let moved = is_position_different(self.geometry_info.position, *position);

        if self.is_kind_of(KindOf::StickToTerrainSlope) {
            let services = self
                .owning_world_services()
                .or_else(crate::system::engine_stores::try_active_services);
            let terrain_handle = services
                .as_ref()
                .map(|services| Arc::clone(services.terrain()))
                .unwrap_or_else(crate::terrain::get_terrain_logic);
            let terrain = terrain_handle
                .try_read()
                .map_err(|_| "Object position terrain is already borrowed")?;
            let mut matrix = Matrix3D::IDENTITY;
            terrain.align_on_terrain(self.geometry_info.angle, position, true, &mut matrix);
            let extent = active_compatibility_extent(&terrain);
            drop(terrain);
            self.set_transform_matrix_with_pose_and_extent(
                &matrix,
                None,
                None,
                &mut |id| crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(id),
                &mut Self::calculate_height_above_terrain,
                |_| Some(extent),
            );
            return Ok(());
        }
        // C++ only refreshes OFF_MAP when isPosDifferent crosses its 0.01
        // threshold. Resolve the owning terrain first so contention cannot
        // leave a committed pose paired with a stale status bit.
        let extent = if moved {
            checked_receiving_terrain_extent(self)?
        } else {
            None
        };
        self.transform_matrix.w_axis = glam::Vec4::new(position.x, position.y, position.z, 1.0);
        self.geometry_info.position = position.clone();
        self.geometry_info.height_above_terrain = self.calculate_height_above_terrain();
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

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable) = drawable.write() {
                let mut matrix = crate::drawable::Drawable::get_transform(&*drawable);
                matrix.w_axis = glam::Vec4::new(position.x, position.y, position.z, 1.0);
                drawable.set_transform(matrix);
            }
        }
        if moved {
            self.set_trigger_area_flags_for_change_in_position();
            self.react_contain_to_transform();
            // Object.cpp:1764-1771 — same OFF_MAP refresh as the transform path.
            if let Some(extent) = extent.as_ref() {
                self.refresh_off_map_status(extent);
            }
        }

        Ok(())
    }

    pub(crate) fn note_main_turret_yaw_and_redeploy(&mut self, yaw: f32) {
        self.cached_main_turret_yaw = yaw;
        self.cached_main_turret_valid = true;
        self.react_contain_to_transform();
    }

    pub(crate) fn react_to_non_main_turret_turn(&mut self) {
        // C++ reactToTurretChange notifies contain on any turret rotation, not only the main yaw cache.
        self.react_contain_to_transform();
    }
    fn react_contain_to_transform(&mut self) {
        let use_turret = self
            .contain
            .as_ref()
            .and_then(|contain| contain.try_lock().ok())
            .is_some_and(|guard| guard.passengers_in_turret());
        let ai_held = use_turret
            && self
                .get_ai_update_interface()
                .is_some_and(|ai| ai.try_lock().is_err());
        let bones = if use_turret && self.cached_main_turret_valid {
            let yaw = self.cached_main_turret_yaw;
            let mut turret_bones = Vec::new();
            for index in 1..=32 {
                let name = format!("FIREPOINT{index:02}");
                let Some(matrix) = self.turret_firepoint_matrix_at_yaw(&name, yaw) else {
                    break;
                };
                turret_bones.push(matrix);
            }
            Some(turret_bones)
        } else if use_turret && ai_held {
            None
        } else if use_turret {
            let mut turret_bones = Vec::new();
            for index in 1..=32 {
                let name = format!("FIREPOINT{index:02}");
                let (found, _, matrix) =
                    self.get_single_logical_bone_position_on_turret(TurretType::Primary, &name);
                if !found {
                    break;
                }
                turret_bones.push(matrix);
            }
            Some(turret_bones)
        } else {
            Some(self.get_multi_logical_bone_matrix("FIREPOINT", 32))
        };
        let position = self.geometry_info.position;
        if let Some(bones) = bones {
            if let Some(contain) = &self.contain {
                if let Ok(mut guard) = contain.try_lock() {
                    if let Some(hidden) = guard.drawable_hidden_after_transform() {
                        if let Some(drawable) = self.get_drawable() {
                            drawable.set_drawable_hidden(hidden);
                        }
                    }
                    guard.redeploy_riders_at(&position, &bones);
                }
            }
        } else if let Some(contain) = &self.contain {
            if let Ok(guard) = contain.try_lock() {
                if let Some(hidden) = guard.drawable_hidden_after_transform() {
                    if let Some(drawable) = self.get_drawable() {
                        drawable.set_drawable_hidden(hidden);
                    }
                }
            }
        }
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
        self.set_trigger_area_flags_with_context(None, &mut |id| {
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(id);
        });
    }

    fn set_trigger_area_flags_with_context(
        &mut self,
        frame: Option<UnsignedInt>,
        changed: &mut dyn FnMut(ObjectID),
    ) {
        self.set_trigger_area_flags_with_observations(
            frame,
            changed,
            &mut |object| {
                if let Some(client) = crate::helpers::TheGameClient::get() {
                    client.notify_terrain_object_moved_from(object);
                }
            },
            None,
        );
    }

    fn set_trigger_area_flags_with_observations(
        &mut self,
        frame: Option<UnsignedInt>,
        changed: &mut dyn FnMut(ObjectID),
        notify_terrain_moved: &mut dyn FnMut(&Object),
        triggers: Option<Vec<Arc<PolygonTrigger>>>,
    ) {
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
            notify_terrain_moved(self);
        }

        // C++ lines 2565-2568: Update pathfinder position
        if self.get_ai_update_interface().is_some() {
            // TheAI->pathfinder()->updatePos(this, getPosition()) - handled by AI system
        }

        let now = frame.unwrap_or_else(crate::helpers::TheGameLogic::get_frame);

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
                    changed(self.id);
                }
            }
        }

        // C++ line 2593: Update integer position
        self.i_pos = new_i_pos;

        // C++ lines 2595-2651: Check for newly entered trigger areas.
        // Iterate every PolygonTrigger, not only already-tracked ones.
        match triggers {
            Some(triggers) => self.enter_polygon_triggers(now, changed, triggers),
            None => self.enter_untracked_polygon_triggers(now, changed),
        }
    }

    /// C++ Object.cpp:2615-2657 — walk `PolygonTrigger::getFirstPolygonTrigger()`.
    fn enter_untracked_polygon_triggers(
        &mut self,
        now: UnsignedInt,
        changed: &mut dyn FnMut(ObjectID),
    ) {
        let terrain_owner_handle = crate::terrain::get_terrain_logic();
        let Ok(terrain) = terrain_owner_handle.read() else {
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

        self.enter_polygon_triggers(now, changed, triggers);
    }

    fn enter_polygon_triggers(
        &mut self,
        now: UnsignedInt,
        changed: &mut dyn FnMut(ObjectID),
        triggers: Vec<Arc<PolygonTrigger>>,
    ) {
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
            changed(self.id);
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

    pub(crate) fn collision_geometry_from_bounds(
        info: &crate::common::GeometryInfo,
        template_type: Option<game_engine::system::geometry::GeometryType>,
    ) -> crate::object::collide::collision_geometry::GeometryInfo {
        let dx = info.bounds().max.x - info.bounds().min.x;
        let dy = info.bounds().max.y - info.bounds().min.y;
        let dz = info.bounds().max.z - info.bounds().min.z;
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
                .with_z_height(height)
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

    /// Set object orientation (stored on geometry info; rendering updates occur elsewhere).
    pub fn set_orientation(&mut self, angle: Real) -> Result<(), String> {
        let position = self.transform_matrix.w_axis.truncate();
        let mut matrix = Matrix3D::from_translation(position) * Matrix3D::from_rotation_z(angle);
        let position_changed = is_position_different(self.geometry_info.position, position);
        let extent = if self.is_kind_of(KindOf::StickToTerrainSlope) {
            let services = self
                .owning_world_services()
                .or_else(crate::system::engine_stores::try_active_services);
            let terrain_handle = services
                .as_ref()
                .map(|services| Arc::clone(services.terrain()))
                .unwrap_or_else(crate::terrain::get_terrain_logic);
            let terrain = terrain_handle
                .try_read()
                .map_err(|_| "Object orientation terrain is already borrowed")?;
            terrain.align_on_terrain(angle, &position, true, &mut matrix);
            position_changed.then(|| active_compatibility_extent(&terrain))
        } else if position_changed {
            checked_receiving_terrain_extent(self)?
        } else {
            None
        };
        // Thing.cpp:174-219 aligns the matrix but retains its pre-alignment
        // position and the existing altitude caches.
        let height = self.geometry_info.height_above_terrain;
        self.set_transform_matrix_with_pose_and_extent(
            &matrix,
            Some((position, normalize_orientation_angle(angle))),
            None,
            &mut |id| crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(id),
            &mut |_| height,
            |_| extent,
        );
        Ok(())
    }

    /// C++ `Object::isOffMap` — the private OFF_MAP status bit, updated by
    /// the transform paths and `friend_notifyOfNewMapBoundary`.
    pub fn is_off_map(&self) -> bool {
        self.private_status & (ObjectPrivateStatusBits::OffMap as u8) != 0
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
        self.transform_matrix
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

    /// Same remove/change/update ordering as setLayer, against the admitted
    /// factory's pathfinder and terrain rather than selecting an active world.
    pub(crate) fn set_layer_with_services(
        &mut self,
        layer: PathfindLayerEnum,
        pf: &mut crate::ai::Pathfinder,
        terrain: &crate::terrain::TerrainLogic,
    ) {
        if layer == self.layer {
            return;
        }
        let old = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(self.layer as u32);
        pf.remove_pos_cells(self.id, 0, true, old);
        self.layer = layer;
        let cell = crate::ai::pathfind_astar::GridCoord::from_world(self.get_position());
        let interacts = terrain.object_interacts_with_bridge_end(
            self,
            crate::path::PathfindLayerEnum::from_u32(layer as u32),
        );
        pf.update_pos_cells(
            cell,
            self.id,
            crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32),
            0,
            true,
            interacts,
        );
    }

    /// Best-effort C++ Pathfinder::removePos / updatePos around a layer change.
    /// No-ops when AI/pathfinder is not constructed (unit tests, early boot).
    pub(super) fn sync_pathfinder_pos(&self, remove_only: bool) {
        let ai_store = crate::ai::the_ai();
        let Ok(ai_guard) = ai_store.read() else {
            return;
        };
        let Some(pathfinder) = ai_guard.pathfinder() else {
            return;
        };
        let Ok(mut pf) = pathfinder.write() else {
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
            let terrain_owner_handle = crate::terrain::get_terrain_logic();
            let interacts = terrain_owner_handle
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
    use std::sync::{Arc, RwLock};

    #[test]
    fn owned_transform_uses_strict_active_terrain_edges_for_off_map() {
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.heightmap = vec![0; 100];
        map.boundaries = vec![crate::common::ICoord2D::new(10, 10)];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_data(map);
        let mut object = Object::new_test(0x00B0_1B01, 100.0);
        for (x, y, expected) in [
            (0.0, 50.0, true),
            (50.0, 50.0, false),
            (100.0, 50.0, true),
            (50.0, 100.0, true),
        ] {
            object.geometry_info.position = Coord3D::new(x, y, 0.0);
            object
                .finish_owned_transform_with_services(
                    OwnedObjectTransform {
                        position_changed: true,
                        pose_changed: true,
                    },
                    &terrain,
                    0,
                    &mut |_| {},
                    &mut |_| {},
                )
                .unwrap();
            assert_eq!(
                object.private_status & (ObjectPrivateStatusBits::OffMap as u8) != 0,
                expected,
                "position ({x}, {y})"
            );
        }
    }

    #[test]
    fn is_off_map_reads_the_canonical_private_status_bit() {
        let mut object = Object::new_test(0x00B0_1B20, 100.0);
        assert!(!object.is_off_map());
        object.private_status |= ObjectPrivateStatusBits::OffMap as u8;
        assert!(object.is_off_map());
        object.private_status &= !(ObjectPrivateStatusBits::OffMap as u8);
        assert!(!object.is_off_map());
    }

    /// Object.cpp:1752-1774 — every pose writer refreshes OFF_MAP from the
    /// receiving world's active extent once a component moves past 0.01.
    #[test]
    fn pose_writers_refresh_off_map_from_the_receiving_extent() {
        let _lock = crate::test_sync::lock();
        fn stores_with_active_cells(size: i32) -> Arc<crate::system::engine_stores::EngineStores> {
            let mut map = crate::system::map_loader::MapData::new();
            map.width = 10;
            map.height = 10;
            map.heightmap = vec![0; 100];
            map.boundaries = vec![
                crate::common::ICoord2D::new(10, 10),
                crate::common::ICoord2D::new(size, size),
            ];
            let stores = crate::system::engine_stores::new_for_world();
            {
                let mut terrain = stores.services().terrain().write().unwrap();
                terrain.load_map_data(map);
                terrain.restore_active_boundary_from_snapshot(1);
            }
            stores
        }

        // Active extent 50 units; maximum pathfind extent 100 units.
        let small = stores_with_active_cells(5);
        let mut object = crate::system::engine_stores::with_active_stores(&small, || {
            let mut object = Object::new_test(0x00B0_1B21, 100.0);
            object.set_position(&Coord3D::new(25.0, 25.0, 0.0)).unwrap();
            assert!(!object.is_off_map());

            // Inside the maximum extent but outside the active boundary:
            // C++ compares against getExtent (active), not the maximum.
            object.set_position(&Coord3D::new(75.0, 75.0, 0.0)).unwrap();
            assert!(object.is_off_map());

            object.set_position(&Coord3D::new(49.998, 25.0, 0.0)).unwrap();
            assert!(!object.is_off_map());

            // A sub-0.01 component move does not satisfy isPosDifferent
            // (Object.cpp:1683 strict >0.01), so this edge crossing leaves
            // the bit unchanged even though the point is now outside.
            object.set_position(&Coord3D::new(50.006, 25.0, 0.0)).unwrap();
            assert!(!object.is_off_map());

            object.set_position(&Coord3D::new(25.0, 25.0, 0.0)).unwrap();
            assert!(!object.is_off_map());

            // The matrix writer refreshes through the same path.
            let mut matrix = object.get_transform_matrix();
            matrix.w_axis = glam::Vec4::new(75.0, 75.0, 0.0, 1.0);
            object.set_transform_matrix(&matrix);
            assert!(object.is_off_map());
            object.bind_owner_world_for_test(small.services());
            object
        });

        // A same-ID peer in another world keeps its own bit and boundary.
        let large = stores_with_active_cells(10);
        let mut peer = crate::system::engine_stores::with_active_stores(&large, || {
            let mut peer = Object::new_test(0x00B0_1B21, 100.0);
            peer.set_position(&Coord3D::new(75.0, 75.0, 0.0)).unwrap();
            assert!(!peer.is_off_map());
            peer.bind_owner_world_for_test(large.services());
            peer
        });
        crate::system::engine_stores::with_active_stores(&large, || {
            object.set_position(&Coord3D::new(76.0, 75.0, 0.0)).unwrap();
            assert!(object.is_off_map());
        });
        crate::system::engine_stores::with_active_stores(&small, || {
            let mut matrix = peer.get_transform_matrix();
            matrix.w_axis = glam::Vec4::new(76.0, 75.0, 0.0, 1.0);
            peer.set_transform_matrix(&matrix);
            assert!(!peer.is_off_map());
        });
    }

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

    #[test]
    fn moving_registered_infantry_while_write_locked_does_not_reread_registry() {
        let _lock = crate::test_sync::lock();
        let object_id = 0x00B0_1B21;
        let mut template = DefaultThingTemplate::new("RegisteredInfantry".to_string());
        template.add_kind_of(KindOf::Infantry);
        let object = Arc::new(RwLock::new(Object::new_test_from_template(
            object_id,
            100.0,
            Arc::new(template),
        )));
        crate::object::registry::OBJECT_REGISTRY.register_object(object_id, &object);

        // This mirrors GarrisonContain::move_objects_with_me: the caller holds
        // the passenger's write guard while Object::set_position notifies the
        // terrain client. The notification must use this borrow, not lock the
        // registry entry again.
        object
            .write()
            .expect("object write lock")
            .set_position(&Coord3D::new(5.0, 5.0, 0.0))
            .expect("move infantry");

        crate::object::registry::OBJECT_REGISTRY.unregister_object(object_id);
    }
    #[test]
    fn borrowed_trigger_context_keeps_same_id_frames_and_notifications_independent() {
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                concat!(
                    module_path!(),
                    "::borrowed_trigger_context_keeps_same_id_frames_and_notifications_independent"
                )
                .strip_prefix("gamelogic::")
                .unwrap(),
                "GENERALS_TRIGGER_CONTEXT_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        // C++ Object.cpp:2589-2616 reads the driving clock, checks the old
        // integer position, marks exit, and synchronously notifies GameLogic.
        let _lock = crate::test_sync::lock();
        let id = 0x00B0_1B22;
        let make_object = || {
            let mut obj = Object::new_test(id, 100.0);
            obj.set_prepared_position(&Coord3D::new(20.0, 20.0, 0.0));
            obj.i_pos = ICoord3D::new(20, 20, 0);
            obj.num_trigger_areas_active = 1;
            obj.trigger_info[0].trigger = Some(Arc::new(square(4243, "BorrowedExitArea")));
            obj.trigger_info[0].is_inside = true;
            obj.trigger_info[0].entered = false;
            obj.trigger_info[0].exited = false;
            obj.entered_or_exited_frame = 0;
            obj
        };
        let mut first = make_object();
        let mut second = make_object();
        let mut first_notifications = Vec::new();
        let mut second_notifications = Vec::new();
        let transform = Matrix3D::from_translation(Coord3D::new(21.0, 20.0, 0.0));

        first.set_transform_matrix_with_trigger_context(&transform, Some(7), &mut |object_id| {
            first_notifications.push(object_id);
        });
        assert!(first.trigger_info[0].exited);
        assert!(!first.trigger_info[0].is_inside);
        assert_eq!(first.entered_or_exited_frame, 7);
        assert_eq!(first_notifications.first(), Some(&id));
        assert!(!second.trigger_info[0].exited);
        assert_eq!(second.entered_or_exited_frame, 0);

        second.set_transform_matrix_with_trigger_context(&transform, Some(19), &mut |object_id| {
            second_notifications.push(object_id);
        });
        assert!(second.trigger_info[0].exited);
        assert_eq!(second.entered_or_exited_frame, 19);
        assert_eq!(second_notifications.first(), Some(&id));
        assert_eq!(first.entered_or_exited_frame, 7);
    }
}
