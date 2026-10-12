//! Object.cpp1794–1838 / PhysicsUpdate.cpp299–391, using the driving owner.
use super::*;
use game_engine::common::random_value::RandomState;

impl PhysicsBehaviorUpdate {
    pub(in crate::object) fn validate_shock_at(&self, owner: &GameObject) -> Result<(), String> {
        let h = &self.physics_handle;
        if self.object_id != owner.get_id() || owner.get_contain().is_some() {
            return Err("shock requires its supplied owner without ambient cargo lookup".into());
        }
        if !h.state.mass.is_finite()
            || h.state.mass <= 0.0
            || !h.module_data.shock_resistance.is_finite()
            || !h.module_data.shock_max_yaw.is_finite()
            || !h.module_data.shock_max_pitch.is_finite()
            || !h.module_data.shock_max_roll.is_finite()
        {
            return Err("nonfinite shock Physics configuration".into());
        }
        Ok(())
    }
    pub(in crate::object) fn prepare_shock_at(
        &mut self,
        force: &Coord3D,
        owner: &GameObject,
        frame: u32,
    ) -> Result<bool, String> {
        self.validate_shock_at(owner)?;
        if !force.is_finite() {
            return Err("nonfinite shock force".into());
        }
        let h = &mut self.physics_handle;
        let resisted = *force * (1.0 - h.module_data.shock_resistance.clamp(0.0, 1.0));
        h.apply_force_with_obj_at(&resisted, Some(owner), frame, false);
        Ok(!h.state.has_flag(FLAG_IS_IN_UPDATE))
    }
    pub(in crate::object) fn prepare_random_rotation_at(
        &mut self,
        owner: &GameObject,
        random: &mut RandomState,
    ) -> Result<bool, String> {
        self.validate_shock_at(owner)?;
        let h = &mut self.physics_handle;
        if h.state.has_flag(FLAG_STICK_TO_GROUND) {
            return Ok(false);
        }
        h.state.set_flag(FLAG_ALLOW_BOUNCE, true);
        h.state.yaw_rate += h.module_data.shock_max_yaw * random.next_real(-1.0, 1.0);
        h.state.pitch_rate += h.module_data.shock_max_pitch * random.next_real(-1.0, 1.0);
        h.state.roll_rate += h.module_data.shock_max_roll * random.next_real(-1.0, 1.0);
        h.update_pitch_roll_yaw_flag();
        Ok(!h.state.has_flag(FLAG_IS_IN_UPDATE))
    }
    pub(in crate::object) fn finish_stunned_at(
        &mut self,
        owner: &mut GameObject,
    ) -> Result<(), String> {
        self.validate_shock_at(owner)?;
        self.physics_handle.state.set_flag(FLAG_IS_STUNNED, true);
        owner.set_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
        Ok(())
    }
}
