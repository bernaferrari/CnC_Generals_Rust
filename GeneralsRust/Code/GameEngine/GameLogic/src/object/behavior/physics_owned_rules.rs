//! Immutable PhysicsUpdate rules resolved by the receiving match's content owner.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsRules {
    pub ground_stiffness: f32,
    pub structure_stiffness: f32,
    pub default_structure_rubble_height: f32,
}
impl PhysicsRules {
    pub fn from_global_data(data: &game_engine::common::global_data::GlobalData) -> Self {
        Self {
            ground_stiffness: data.ground_stiffness,
            structure_stiffness: data.structure_stiffness,
            default_structure_rubble_height: data.default_structure_rubble_height,
        }
    }
    pub(crate) fn validate(self) -> Result<(), String> {
        if !self.ground_stiffness.is_finite()
            || !self.structure_stiffness.is_finite()
            || !self.default_structure_rubble_height.is_finite()
        {
            return Err("nonfinite supplied Physics rules".into());
        }
        Ok(())
    }
}
