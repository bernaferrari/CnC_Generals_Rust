//! Receiving-service witness shared by the actual Tank/Truck module tests.
use super::*;

#[derive(Default)]
pub(crate) struct Receiver {
    pub(crate) destroyed: Vec<u32>,
}
impl Receiver {
    pub(crate) fn context() -> NativeStateParticleContext {
        NativeStateParticleContext {
            conditions: ModelConditionFlags::empty(),
            instance_scale: 1.0,
            state_particles: true,
            drawable_id: 17,
            hidden: false,
            fully_obscured: false,
            creation_frame: 3,
        }
    }
}
impl ClientParticleStateRuntime for Receiver {
    fn standard_public_bones(&self) -> Vec<String> {
        Vec::new()
    }
    fn pristine_bone(
        &mut self,
        _model: &str,
        _scale: f32,
        _frame: i32,
        _name: &str,
    ) -> Option<(i32, Matrix3D)> {
        None
    }
    fn sub_object_names(&mut self, _model: &str) -> Vec<String> {
        Vec::new()
    }
    fn destroy_particle_system(&mut self, id: u32) {
        self.destroyed.push(id);
    }
    fn set_particle_system_running(&mut self, _id: u32, _running: bool) {}
    fn create_particle_system(
        &mut self,
        template: &str,
        frame: u32,
    ) -> Result<Option<u32>, String> {
        assert_eq!(template, "Smoke");
        assert_eq!(frame, 3);
        Ok(Some(91))
    }
    fn model_space_bone(&mut self, model: &str, bone: &str) -> Option<(i32, Matrix3D)> {
        assert_eq!(model, "Vehicle");
        assert_eq!(bone, "FXBONE");
        Some((1, Matrix3D::IDENTITY))
    }
    fn configure_particle_system(
        &mut self,
        id: u32,
        drawable_id: u32,
        _bone_index: i32,
        _local: &Matrix3D,
        hidden: bool,
    ) {
        assert_eq!((id, drawable_id, hidden), (91, 17, false));
    }
}
pub(crate) fn model_data() -> W3DModelDrawModuleData {
    let mut data = W3DModelDrawModuleData::new();
    let mut state = ModelConditionInfo::new();
    state.model_name = AsciiString::from("Vehicle");
    state.conditions_yes.push(ModelConditionFlags::empty());
    state.public_bones.push(AsciiString::from("FXBONE"));
    state.particle_sys_bones.push(ParticleSysBoneInfo {
        bone_name: AsciiString::from("FXBONE"),
        particle_system: AsciiString::from("Smoke"),
    });
    data.condition_states.push(state);
    data
}
