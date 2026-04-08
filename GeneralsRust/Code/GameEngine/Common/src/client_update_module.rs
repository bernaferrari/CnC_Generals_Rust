// FILE: client_update_module.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/ClientUpdateModule.h

pub use crate::common::thing::draw_module::DrawableModuleTrait;
pub use crate::common::thing::module::ModuleData;

pub trait ClientUpdateModuleData: ModuleData {}

impl<T: ModuleData + ?Sized> ClientUpdateModuleData for T {}

pub trait ClientUpdateModule: DrawableModuleTrait {
    fn client_update(&mut self);
}
