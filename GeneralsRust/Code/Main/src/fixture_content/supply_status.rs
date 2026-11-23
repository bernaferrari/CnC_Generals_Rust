use crate::game_logic::{
    NativeBodyVisualChange, NativeFactoryDrawableSource, NativeObjectRetirement, NativeSlaveSource,
    NativeViewEffects, ObjectId,
};
use crate::presentation_frame::camera::build::SlaveCameraSample;
use game_client::{
    core::game_client::NativeDrawableBirthContext, drawable::drawable::NativeAmbientAudio,
};
use game_engine::common::{
    global_data::GlobalData,
    thing::{module_factory::ModuleFactory, thing_template::ModuleInfo},
};

pub(crate) struct SupplyStatus {
    pub(crate) object_id: ObjectId,
    pub(crate) maximum: i32,
    pub(crate) current: i32,
    pub(crate) condition_bits: u128,
    pub(crate) draw_modules: usize,
}

#[derive(Default)]
pub(crate) struct SupplyStatusRecorder {
    pub(crate) calls: Vec<SupplyStatus>,
}

impl NativeViewEffects for SupplyStatusRecorder {
    fn local_player_id(&self) -> u32 {
        0
    }

    fn host_epoch(&self) -> u64 {
        1
    }

    fn sample_slave(&mut self, _: &NativeSlaveSource, _: &str) -> SlaveCameraSample {
        SlaveCameraSample::MissingDrawable
    }

    fn body_visual_changed(
        &mut self,
        _: &NativeBodyVisualChange,
        _: Option<&mut dyn NativeAmbientAudio>,
    ) -> Result<bool, String> {
        Ok(false)
    }

    fn retire_physical_object_drawable(
        &mut self,
        _: &NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn admit_factory_drawable(
        &mut self,
        _: NativeFactoryDrawableSource,
        _: NativeDrawableBirthContext,
        _: &mut ModuleFactory,
    ) -> Result<(), String> {
        Ok(())
    }

    fn update_drawable_supply_status(
        &mut self,
        source: &NativeSlaveSource,
        draws: &ModuleInfo,
        _: &parking_lot::RwLock<GlobalData>,
        maximum: i32,
        current: i32,
    ) -> bool {
        self.calls.push(SupplyStatus {
            object_id: ObjectId(source.identity.object_id()),
            maximum,
            current,
            condition_bits: source.condition_bits,
            draw_modules: draws.iter().count(),
        });
        true
    }
}
