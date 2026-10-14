use super::*;

impl Default for PhysicsBehaviorModuleData {
    fn default() -> Self {
        let gravity = global_data::read_safe()
            .map(|data| data.gravity)
            .unwrap_or(-1.0);
        Self::with_gravity(gravity)
    }
}

impl PhysicsBehaviorModuleData {
    /// C++ PhysicsBehaviorModuleData defaults with the owning content's gravity.
    /// Constructing definitions through this path never observes ambient state.
    pub fn with_gravity(gravity: Real) -> Self {
        Self {
            base: BehaviorModuleData::default(),
            mass: DEFAULT_MASS,
            shock_resistance: 0.0,
            shock_max_yaw: DEFAULT_SHOCK_YAW,
            shock_max_pitch: DEFAULT_SHOCK_PITCH,
            shock_max_roll: DEFAULT_SHOCK_ROLL,
            forward_friction: DEFAULT_FORWARD_FRICTION,
            lateral_friction: DEFAULT_LATERAL_FRICTION,
            z_friction: DEFAULT_Z_FRICTION,
            aerodynamic_friction: DEFAULT_AERO_FRICTION,
            center_of_mass_offset: 0.0,
            kill_when_resting_on_ground: false,
            allow_bouncing: false,
            allow_collide_force: true,
            min_fall_speed_for_damage: (2.0 * gravity * 40.0).abs().sqrt(),
            fall_height_damage_factor: 1.0,
            pitch_roll_yaw_factor: 2.0,
            vehicle_crashes_into_building_weapon_template: AsciiString::from(
                "VehicleCrashesIntoBuildingWeapon",
            ),
            vehicle_crashes_into_non_building_weapon_template: AsciiString::from(
                "VehicleCrashesIntoNonBuildingWeapon",
            ),
        }
    }
}

impl PhysicsBehaviorModuleData {
    /// Parse one authored field using the owning content's gravity.
    /// Other fields retain the same field table and parsers as `parse_from_ini`.
    pub fn apply_ini_field_with_gravity(
        &mut self,
        field: &str,
        value: &str,
        gravity: Real,
    ) -> Result<(), INIError> {
        if !gravity.is_finite() {
            return Err(INIError::InvalidData);
        }
        let descriptor = PHYSICS_BEHAVIOR_FIELDS
            .iter()
            .find(|descriptor| descriptor.token.eq_ignore_ascii_case(field))
            .ok_or(INIError::InvalidData)?;
        let tokens: Vec<_> = value.split_whitespace().collect();
        if descriptor.token == "MinFallHeightForDamage" {
            let height = INI::parse_real(required_value(&tokens)?)?;
            let speed = (2.0 * gravity * height).abs().sqrt();
            if !height.is_finite() || !speed.is_finite() {
                return Err(INIError::InvalidData);
            }
            self.min_fall_speed_for_damage = speed;
            return Ok(());
        }
        // Parse into a detached definition so rejected values leave the caller intact.
        let mut candidate = self.clone();
        (descriptor.parse)(&mut INI::new(), &mut candidate, &tokens)?;
        let scalars = [
            candidate.mass,
            candidate.shock_resistance,
            candidate.shock_max_yaw,
            candidate.shock_max_pitch,
            candidate.shock_max_roll,
            candidate.forward_friction,
            candidate.lateral_friction,
            candidate.z_friction,
            candidate.aerodynamic_friction,
            candidate.center_of_mass_offset,
            candidate.min_fall_speed_for_damage,
            candidate.fall_height_damage_factor,
            candidate.pitch_roll_yaw_factor,
        ];
        if scalars.iter().any(|value| !value.is_finite()) {
            return Err(INIError::InvalidData);
        }
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_gravity_defaults_preserve_cpp_values() {
        for gravity in [-1.0, -3.5, 0.0, 2.0] {
            let data = PhysicsBehaviorModuleData::with_gravity(gravity);
            assert_eq!(
                data.min_fall_speed_for_damage,
                (2.0 * gravity * 40.0).abs().sqrt()
            );
            assert_eq!(data.mass, DEFAULT_MASS);
            assert_eq!(data.pitch_roll_yaw_factor, 2.0);
            assert!(data.allow_collide_force);
            assert!(!data.allow_bouncing);
            assert!(!data.kill_when_resting_on_ground);
        }
    }

    #[test]
    fn supplied_gravity_fields_reuse_cpp_friction_and_height_parsers() {
        let mut data = PhysicsBehaviorModuleData::with_gravity(-2.0);
        data.apply_ini_field_with_gravity("minfallheightfordamage", "80", -3.0)
            .unwrap();
        assert_eq!(data.min_fall_speed_for_damage, (480.0_f32).sqrt());
        data.apply_ini_field_with_gravity("ForwardFriction", "6", -3.0)
            .unwrap();
        assert_eq!(data.forward_friction, 6.0 * SECONDS_PER_LOGICFRAME_REAL);
        data.apply_ini_field_with_gravity("Mass", "5", -3.0)
            .unwrap();
        assert_eq!(data.mass, 5.0);
    }

    #[test]
    fn rejected_owned_fields_leave_definition_intact() {
        let mut data = PhysicsBehaviorModuleData::with_gravity(-2.0);
        let mass = data.mass;
        let speed = data.min_fall_speed_for_damage;
        for (field, value, gravity) in [
            ("Mass", "0", -2.0),
            ("Unknown", "1", -2.0),
            ("Mass", "NaN", -2.0),
            ("MinFallHeightForDamage", "inf", -2.0),
            ("Mass", "5", f32::NAN),
        ] {
            assert!(
                data.apply_ini_field_with_gravity(field, value, gravity)
                    .is_err()
            );
            assert_eq!(data.mass, mass);
            assert_eq!(data.min_fall_speed_for_damage, speed);
        }
    }
}
