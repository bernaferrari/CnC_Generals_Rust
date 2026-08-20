//! Shared `SpecialPowerModuleInterface` forwarding for C++ subclass modules.
//!
//! C++ subclasses inherit `SpecialPowerModule` and override `doSpecialPower*`.
//! Object dispatch downcasts to the concrete module, then calls the virtual.
//! Each Rust subclass owns a `base_module` so recharge/EVA/view-object still run.

use crate::common::{Coord3D, ObjectID};
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::special_power_module::{
    SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleInterface, Waypoint,
};
use crate::object::special_power_template::{AudioEventRts, SpecialPowerTemplate};
use std::sync::Arc;

/// Forward the non-fire SpecialPowerModuleInterface methods to `base_module`.
macro_rules! impl_special_power_subclass {
    ($ty:ty) => {
        impl $crate::object::special_power_module::SpecialPowerModuleInterface for $ty {
            fn is_module_for_power(
                &self,
                special_power_template: &$crate::object::special_power_template::SpecialPowerTemplate,
            ) -> bool {
                $crate::object::special_power_module::SpecialPowerModuleInterface::is_module_for_power(
                    &self.base_module,
                    special_power_template,
                )
            }

            fn get_percent_ready(&self) -> f32 {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_percent_ready(
                    &self.base_module,
                )
            }

            fn get_power_name(&self) -> String {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_power_name(
                    &self.base_module,
                )
            }

            fn get_special_power_template_full(
                &self,
            ) -> Option<std::sync::Arc<$crate::object::special_power_template::SpecialPowerTemplate>>
            {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_special_power_template_full(
                    &self.base_module,
                )
            }

            fn get_required_science(&self) -> $crate::common::science::ScienceType {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_required_science(
                    &self.base_module,
                )
            }

            fn on_special_power_creation(&mut self) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::on_special_power_creation(
                    &mut self.base_module,
                );
                self.dispatch_on_special_power_creation();
            }

            fn set_ready_frame(
                &mut self,
                frame: $crate::object::special_power_module::FrameCount,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::set_ready_frame(
                    &mut self.base_module,
                    frame,
                )
            }

            fn pause_countdown(&mut self, pause: bool) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::pause_countdown(
                    &mut self.base_module,
                    pause,
                )
            }

            fn do_special_power(
                &mut self,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                self.dispatch_do_special_power(command_options);
            }

            fn do_special_power_at_object(
                &mut self,
                object_id: $crate::object::special_power_module::ObjectId,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                self.dispatch_do_special_power_at_object(object_id, command_options);
            }

            fn do_special_power_at_location(
                &mut self,
                location: &$crate::common::Coord3D,
                angle: f32,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                self.dispatch_do_special_power_at_location(location, angle, command_options);
            }

            fn do_special_power_using_waypoints(
                &mut self,
                waypoint: &$crate::object::special_power_module::Waypoint,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_using_waypoints(
                    &mut self.base_module,
                    waypoint,
                    command_options,
                )
            }

            fn mark_special_power_triggered(&mut self, location: Option<&$crate::common::Coord3D>) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::mark_special_power_triggered(
                    &mut self.base_module,
                    location,
                )
            }

            fn start_power_recharge_at(
                &mut self,
                current_frame: $crate::object::special_power_module::FrameCount,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::start_power_recharge_at(
                    &mut self.base_module,
                    current_frame,
                )
            }

            fn get_initiate_sound(
                &self,
            ) -> &$crate::object::special_power_template::AudioEventRts {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_initiate_sound(
                    &self.base_module,
                )
            }

            fn is_script_only(&self) -> bool {
                $crate::object::special_power_module::SpecialPowerModuleInterface::is_script_only(
                    &self.base_module,
                )
            }

            fn get_reference_thing_template(&self) -> Option<String> {
                self.dispatch_reference_thing_template()
            }
        }

        impl $crate::modules::SpecialPowerModuleInterface for $ty {
            fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power(
                    self,
                    $crate::object::special_power_module::SpecialPowerCommandOptions::NONE,
                );
                Ok(())
            }

            fn can_activate(&self) -> bool {
                $crate::modules::SpecialPowerModuleInterface::can_activate(&self.base_module)
            }

            fn get_power_type(&self) -> u32 {
                $crate::modules::SpecialPowerModuleInterface::get_power_type(&self.base_module)
            }

            fn start_power_recharge(
                &mut self,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                $crate::modules::SpecialPowerModuleInterface::start_power_recharge(
                    &mut self.base_module,
                )
            }

            fn get_ready_frame(&self) -> u32 {
                $crate::modules::SpecialPowerModuleInterface::get_ready_frame(&self.base_module)
            }

            fn is_ready(&self) -> bool {
                $crate::modules::SpecialPowerModuleInterface::is_ready(&self.base_module)
            }

            fn get_special_power_template(&self) -> Option<std::sync::Arc<dyn std::any::Any>> {
                $crate::modules::SpecialPowerModuleInterface::get_special_power_template(
                    &self.base_module,
                )
            }

            fn get_special_power_template_full(
                &self,
            ) -> Option<std::sync::Arc<$crate::object::special_power_template::SpecialPowerTemplate>>
            {
                $crate::modules::SpecialPowerModuleInterface::get_special_power_template_full(
                    &self.base_module,
                )
            }

            fn is_module_for_power(
                &self,
                special_power_template: &$crate::object::special_power_template::SpecialPowerTemplate,
            ) -> bool {
                $crate::object::special_power_module::SpecialPowerModuleInterface::is_module_for_power(
                    self,
                    special_power_template,
                )
            }

            fn set_ready_frame(&mut self, frame: u32) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::set_ready_frame(
                    self,
                    frame,
                )
            }

            fn get_power_name(&self) -> String {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_power_name(
                    self,
                )
            }

            fn get_percent_ready(&self) -> f32 {
                $crate::object::special_power_module::SpecialPowerModuleInterface::get_percent_ready(
                    self,
                )
            }

            fn pause_countdown(&mut self, pause: bool) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::pause_countdown(
                    self,
                    pause,
                )
            }

            fn mark_special_power_triggered(&mut self, location: Option<&$crate::common::Coord3D>) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::mark_special_power_triggered(
                    self,
                    location,
                )
            }

            fn on_special_power_creation(&mut self) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::on_special_power_creation(
                    self,
                )
            }

            fn do_special_power(
                &mut self,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power(
                    self,
                    command_options,
                )
            }

            fn do_special_power_at_object(
                &mut self,
                object_id: $crate::common::ObjectID,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_object(
                    self,
                    object_id,
                    command_options,
                )
            }

            fn do_special_power_at_location(
                &mut self,
                location: &$crate::common::Coord3D,
                angle: f32,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_location(
                    self,
                    location,
                    angle,
                    command_options,
                )
            }

            fn do_special_power_using_waypoints(
                &mut self,
                waypoint: &$crate::object::special_power_module::Waypoint,
                command_options: $crate::object::special_power_module::SpecialPowerCommandOptions,
            ) {
                $crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_using_waypoints(
                    self,
                    waypoint,
                    command_options,
                )
            }
        }
    };
}

pub(crate) use impl_special_power_subclass;

/// Default reference-template hook used by subclasses that have none.
pub(crate) fn no_reference_thing_template() -> Option<String> {
    None
}

/// Shared constructor helper for the inherited SpecialPowerModule state.
pub(crate) fn make_base_module(
    owner_object_id: ObjectID,
    data: &crate::object::special_power_module::SpecialPowerModuleData,
) -> SpecialPowerModule {
    SpecialPowerModule::new(owner_object_id, data.clone())
}

/// C++ subclass `xfer`: version then `SpecialPowerModule::xfer`.
pub(crate) fn xfer_special_power_subclass(
    base_module: &mut SpecialPowerModule,
    xfer: &mut dyn game_engine::common::system::Xfer,
    name: &str,
) -> Result<(), String> {
    let mut version: crate::common::XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("{name} xfer version failed: {:?}", e))?;
    game_engine::common::system::Snapshotable::xfer(base_module, xfer)
}

/// C++ subclass `loadPostProcess` extends `SpecialPowerModule::loadPostProcess`.
pub(crate) fn load_post_process_special_power_subclass(
    base_module: &mut SpecialPowerModule,
) -> Result<(), String> {
    game_engine::common::system::Snapshotable::load_post_process(base_module)
}

pub(crate) fn _unused_trait_tokens(
    _m: &SpecialPowerModule,
    _t: Option<Arc<SpecialPowerTemplate>>,
    _a: Option<&AudioEventRts>,
    _w: Option<&Waypoint>,
    _c: Option<&Coord3D>,
    _o: SpecialPowerCommandOptions,
    _e: Option<&dyn EngineSpecialPowerModuleInterface>,
) {
}
