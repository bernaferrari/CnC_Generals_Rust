//! The admitted ordinary ground-locomotion operation. Motion calculations live
//! in Locomotor; this adapter supplies this match's owner, rules and schedulers.
use super::*;
use crate::locomotor::{Locomotor, LocomotorAppearance, LocomotorBehaviorZ};

impl BorrowedAiWorld<'_> {
    fn locomotion_update_proxies(
        &self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &Locomotor,
    ) -> Result<
        (
            crate::object::PhysicsInterfaceHandle,
            game_engine::common::thing::update_module::UpdateModulePtr,
            game_engine::common::thing::update_module::UpdateModulePtr,
        ),
        String,
    > {
        if !matches!(
            locomotor.get_appearance(),
            LocomotorAppearance::TwoLegs | LocomotorAppearance::Other
        ) || locomotor.template.behavior_z != LocomotorBehaviorZ::NoZMotiveForce
        {
            return Err(
                "native path locomotion admits only ordinary ground TWO_LEGS / OTHER".into(),
            );
        }
        if (locomotor.get_appearance() == LocomotorAppearance::TwoLegs
            && locomotor.template.min_speed > 0.0)
            || (locomotor.get_appearance() != LocomotorAppearance::TwoLegs
                && locomotor.template.wander_width_factor != 0.0)
        {
            return Err("native ground path requires zero infantry minimum speed and infantry-only wandering".into());
        }
        let actual_owner = self
            .factory
            .get_object(
                owner
                    .read()
                    .map_err(|_| "locomotion owner poisoned")?
                    .get_id(),
            )
            .and_then(|instance| instance.get_base_object())
            .ok_or("locomotion owner is not admitted to this factory")?;
        if !Arc::ptr_eq(&actual_owner, owner) {
            return Err("locomotion owner belongs to another factory".into());
        }
        let (physics, current_ai, current_physics) = {
            let object = owner.read().map_err(|_| "locomotion owner poisoned")?;
            if object.get_container_id().is_some() {
                return Err("native locomotion does not admit contained owners".into());
            }
            // Physics mass-with-cargo walks this module and resolves cargo
            // through the ambient object registry. Even an empty Contain
            // module is outside this instance-owned operation's admission.
            if object.get_contain().is_some() {
                return Err("native locomotion does not admit owners with Contain modules".into());
            }
            if object.get_body_module().is_none() {
                return Err("native locomotion requires its installed Body module".into());
            }
            if object.is_disabled_by_type(crate::common::DisabledType::Held) {
                return Err("native ground path does not admit held owners".into());
            }
            let physics = object
                .get_physics()
                .ok_or("native locomotion owner has no Physics descriptor")?;
            if self.above_terrain(&object)?
                && physics
                    .access()
                    .map_err(|_| "locomotion Physics poisoned")?
                    .get_allow_to_fall()
            {
                return Err("native ground path does not admit falling airborne owners".into());
            }
            let ai_proxy = object.installed_update_proxy_named("AIUpdateInterface")?;
            let physics_proxy = physics.installed_update_proxy(&object)?;
            (physics, ai_proxy, physics_proxy)
        };
        let current_update = self
            .services
            .current_update
            .as_ref()
            .ok_or("native locomotion requires its driving AI update proxy")?;
        if !Arc::ptr_eq(&current_ai, current_update) {
            return Err("driving update proxy is not this owner's installed AI module".into());
        }
        self.services
            .update_scheduler
            .validate_owned_update_wake(&current_ai)?;
        self.services
            .update_scheduler
            .validate_owned_update_wake(&current_physics)?;
        Ok((physics, current_ai, current_physics))
    }

    /// Run one TWO_LEGS / OTHER, NO_Z_MOTIVE_FORCE path step on the exact factory
    /// Object. Each motive force updates canonical Physics through a short
    /// lease, then admits its actual installed Physics proxy before the next
    /// Physics observation.
    pub(crate) fn move_ground_step(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &mut Locomotor,
        target: Coord3D,
        on_path_distance: f32,
        desired_speed: f32,
        blocked: bool,
        condition: crate::locomotor::BodyDamageType,
        penalty: crate::locomotor::BodyDamageType,
    ) -> Result<(Coord3D, f32, f32), String> {
        if !target.is_finite() || !on_path_distance.is_finite() || !desired_speed.is_finite() {
            return Err("native locomotion requires finite motion inputs".into());
        }
        let (physics, current_update, proxy) = self.locomotion_update_proxies(owner, locomotor)?;
        let (current, angle, condition) = {
            let object = owner.read().map_err(|_| "locomotion owner poisoned")?;
            let actual_condition = object
                .get_body_module()
                .ok_or("native locomotion requires its installed Body module")?
                .lock()
                .map_err(|_| "locomotion body poisoned")?
                .get_damage_state();
            (
                *object.get_position(),
                object.get_orientation(),
                map_body_condition(actual_condition),
            )
        };

        let frame = self.services.frame;
        let gravity = self.services.gravity;
        let terrain = self.services.terrain;
        let pathfinder = &*self.services.pathfinder;
        let mut object = owner.write().map_err(|_| "locomotion owner poisoned")?;
        {
            let mut physics_state = physics
                .access()
                .map_err(|_| "native locomotion Physics poisoned")?;
            locomotor.apply_physics_options(&mut *physics_state);
        }
        let forward_speed = physics
            .access()
            .map_err(|_| "native locomotion Physics poisoned")?
            .get_forward_speed_2d_with_object(&object);

        let mut wake = || {
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    Some(&current_update),
                    frame,
                    frame.wrapping_add(1),
                )
        };
        let mut access = crate::locomotor::LocomotorPhysicsAccess::Installed {
            handle: &physics,
            frame,
            wake: &mut wake,
        };
        let mut change_pose = |object: &mut Object, change| match change {
            crate::locomotor::LocomotorPoseChange::Transform(transform) => object
                .set_transform_matrix_with_services(
                    &transform,
                    terrain,
                    &mut self.services.collision,
                    frame,
                    &mut |id| self.services.changed_trigger_objects.push(id),
                    &mut |owner| self.services.terrain_moved_objects.push(owner.get_id()),
                ),
            crate::locomotor::LocomotorPoseChange::Position(position) => apply_native_position(
                object,
                &position,
                terrain,
                &mut self.services.collision,
                frame,
                &mut self.services.changed_trigger_objects,
                &mut self.services.terrain_moved_objects,
            ),
            crate::locomotor::LocomotorPoseChange::Orientation(angle) => apply_native_orientation(
                object,
                angle,
                terrain,
                &mut self.services.collision,
                frame,
                &mut self.services.changed_trigger_objects,
                &mut self.services.terrain_moved_objects,
            ),
        };
        let mut observations = crate::locomotor::LocomotorMovementObservations {
            frame,
            gravity,
            penalty,
            terrain,
            pathfinder,
            change_pose: &mut change_pose,
        };
        let max_speed = locomotor.get_max_speed_with_penalty(condition, penalty);
        let desired_speed = desired_speed.min(max_speed);
        let result = locomotor.loco_update_move_with_access(
            current,
            angle,
            forward_speed,
            target,
            on_path_distance,
            desired_speed,
            condition,
            1.0,
            blocked,
            Some(access),
            Some(&mut object),
            Some(&mut observations),
        )?;
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::Braking,
            ),
            locomotor.is_braking(),
        );
        let height =
            crate::locomotor::LocomotorMovementObservations::observe_height(&object, terrain);
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::AirborneTarget,
            ),
            height > locomotor.template.airborne_targeting_height as f32,
        );
        Ok(result)
    }

    pub(crate) fn validate_ground_path_step(
        &self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &Locomotor,
    ) -> Result<(), String> {
        self.locomotion_update_proxies(owner, locomotor).map(|_| ())
    }
}

fn map_body_condition(value: crate::common::BodyDamageType) -> crate::locomotor::BodyDamageType {
    match value {
        crate::common::BodyDamageType::Pristine => crate::locomotor::BodyDamageType::Pristine,
        crate::common::BodyDamageType::Damaged => crate::locomotor::BodyDamageType::Damaged,
        crate::common::BodyDamageType::ReallyDamaged => {
            crate::locomotor::BodyDamageType::ReallyDamaged
        }
        crate::common::BodyDamageType::Rubble => crate::locomotor::BodyDamageType::Rubble,
    }
}

fn apply_native_position(
    object: &mut Object,
    position: &Coord3D,
    terrain: &crate::terrain::TerrainLogic,
    collision: &mut crate::object::collide::collision_system::CollisionSystem,
    frame: u32,
    changed: &mut Vec<ObjectID>,
    moved: &mut Vec<ObjectID>,
) -> Result<(), String> {
    let mut matrix = object.get_transform_matrix();
    if object.is_kind_of(KindOf::StickToTerrainSlope) {
        terrain.align_on_terrain(object.get_orientation(), position, true, &mut matrix);
    } else {
        matrix.w_axis = glam::Vec4::new(position.x, position.y, position.z, 1.0);
    }
    object.set_transform_matrix_with_services(
        &matrix,
        terrain,
        collision,
        frame,
        &mut |id| changed.push(id),
        &mut |object| moved.push(object.get_id()),
    )
}

fn apply_native_orientation(
    object: &mut Object,
    angle: f32,
    terrain: &crate::terrain::TerrainLogic,
    collision: &mut crate::object::collide::collision_system::CollisionSystem,
    frame: u32,
    changed: &mut Vec<ObjectID>,
    moved: &mut Vec<ObjectID>,
) -> Result<(), String> {
    let position = *object.get_position();
    let mut matrix =
        glam::Mat4::from_translation(glam::Vec3::new(position.x, position.y, position.z))
            * glam::Mat4::from_rotation_z(angle);
    if object.is_kind_of(KindOf::StickToTerrainSlope) {
        terrain.align_on_terrain(angle, &position, true, &mut matrix);
    }
    object.set_transform_matrix_with_services(
        &matrix,
        terrain,
        collision,
        frame,
        &mut |id| changed.push(id),
        &mut |object| moved.push(object.get_id()),
    )
}
