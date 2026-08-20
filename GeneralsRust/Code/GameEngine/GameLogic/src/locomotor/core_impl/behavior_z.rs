/// Result of C++ `Locomotor::handleBehaviorZ`.
pub struct BehaviorZResult {
    pub lift: Real,
    pub requires_constant: bool,
    pub snapped_z: Option<Real>,
}

impl Locomotor {
    /// Handle Z-axis behavior — snap or lift.
    /// Matches C++ Locomotor::handleBehaviorZ (Locomotor.cpp:2196-2323).
    pub fn handle_behavior_z(
        &self,
        current_pos: Coord3D,
        goal_pos: Coord3D,
        condition: BodyDamageType,
        gravity: Real,
        vel_z: Real,
    ) -> (Real, bool) {
        let result = self.handle_behavior_z_full(current_pos, goal_pos, condition, gravity, vel_z);
        (result.lift, result.requires_constant)
    }

    pub fn handle_behavior_z_full(
        &self,
        current_pos: Coord3D,
        goal_pos: Coord3D,
        condition: BodyDamageType,
        gravity: Real,
        vel_z: Real,
    ) -> BehaviorZResult {
        self.handle_behavior_z_for(
            current_pos,
            goal_pos,
            condition,
            gravity,
            vel_z,
            false,
            crate::common::PathfindLayerEnum::Ground,
        )
    }

    /// C++ `handleBehaviorZ` with DISABLED_HELD + object layer (Locomotor.cpp:2196-2323).
    pub fn handle_behavior_z_for(
        &self,
        current_pos: Coord3D,
        goal_pos: Coord3D,
        condition: BodyDamageType,
        gravity: Real,
        vel_z: Real,
        disabled_held: bool,
        layer: crate::common::PathfindLayerEnum,
    ) -> BehaviorZResult {
        match self.template.behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => BehaviorZResult {
                lift: 0.0,
                requires_constant: false,
                snapped_z: None,
            },
            LocomotorBehaviorZ::SeaLevel => {
                // C++ Locomotor.cpp:2208-2221 — skip snap while DISABLED_HELD;
                // else waterZ if underwater, otherwise getLayerHeight(..., obj->getLayer()).
                if disabled_held {
                    BehaviorZResult {
                        lift: 0.0,
                        requires_constant: true,
                        snapped_z: None,
                    }
                } else {
                    let snapped = TheTerrainLogic::get()
                        .map(|terrain| {
                            let mut water_z = 0.0;
                            if terrain.is_underwater(
                                current_pos.x,
                                current_pos.y,
                                Some(&mut water_z),
                                None,
                            ) {
                                water_z
                            } else {
                                terrain.get_layer_height(current_pos.x, current_pos.y, layer)
                            }
                        })
                        .unwrap_or(current_pos.z);
                    BehaviorZResult {
                        lift: 0.0,
                        requires_constant: true,
                        snapped_z: Some(snapped),
                    }
                }
            }
            LocomotorBehaviorZ::FixedSurfaceRelativeHeight
            | LocomotorBehaviorZ::FixedAbsoluteHeight => {
                let surface_rel =
                    self.template.behavior_z == LocomotorBehaviorZ::FixedSurfaceRelativeHeight;
                let surface_ht = if surface_rel {
                    self.get_surface_ht_at_pt(current_pos.x, current_pos.y)
                } else {
                    0.0
                };
                BehaviorZResult {
                    lift: 0.0,
                    requires_constant: true,
                    snapped_z: Some(self.preferred_height + surface_ht),
                }
            }
            LocomotorBehaviorZ::RelativeToGroundAndBuildings => {
                let surface_ht = crate::object::collide::partition_manager::PARTITION_MANAGER
                    .read()
                    .ok()
                    .map(|pm| pm.get_ground_or_structure_height(current_pos.x, current_pos.y))
                    .unwrap_or_else(|| self.get_surface_ht_at_pt(current_pos.x, current_pos.y));
                BehaviorZResult {
                    lift: 0.0,
                    requires_constant: true,
                    snapped_z: Some(self.preferred_height + surface_ht),
                }
            }
            LocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
                self.lift_relative_to_layer(current_pos, goal_pos, condition, gravity, vel_z, true)
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight | LocomotorBehaviorZ::AbsoluteHeight => {
                self.lift_relative_to_layer(
                    current_pos,
                    goal_pos,
                    condition,
                    gravity,
                    vel_z,
                    false,
                )
            }
        }
    }

    fn lift_relative_to_layer(
        &self,
        current_pos: Coord3D,
        goal_pos: Coord3D,
        condition: BodyDamageType,
        gravity: Real,
        vel_z: Real,
        highest_layer: bool,
    ) -> BehaviorZResult {
        if self.preferred_height == 0.0 && !self.uses_precise_z_pos() {
            return BehaviorZResult {
                lift: 0.0,
                requires_constant: true,
                snapped_z: None,
            };
        }

        let surface_rel = highest_layer
            || self.template.behavior_z == LocomotorBehaviorZ::SurfaceRelativeHeight;
        let surface_ht = if highest_layer {
            TheTerrainLogic::get()
                .map(|terrain| {
                    let layer = terrain.get_highest_layer_for_destination(&current_pos);
                    terrain.get_layer_height(current_pos.x, current_pos.y, layer)
                })
                .unwrap_or(0.0)
        } else if surface_rel {
            self.get_surface_ht_at_pt(current_pos.x, current_pos.y)
        } else {
            0.0
        };

        let mut preferred = self.preferred_height + if surface_rel { surface_ht } else { 0.0 };
        if self.uses_precise_z_pos() {
            preferred = goal_pos.z;
        }
        let delta = preferred - current_pos.z;
        let damped_preferred = current_pos.z + delta * self.preferred_height_damping;
        let lift = self.calc_lift_to_use_at_pt(
            current_pos.z,
            surface_ht,
            damped_preferred,
            vel_z,
            condition,
            gravity,
        );
        BehaviorZResult {
            lift,
            requires_constant: true,
            snapped_z: None,
        }
    }
}
