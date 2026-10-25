//! Installed Laser access is inert; model conditions belong to Drawable phases.
use super::*;
use gamelogic::object::draw::W3DLaserDraw;
use gamelogic::object::draw::w3d_laser_draw::LaserDrawInput;
use gamelogic::object::update::laser_update::LaserUpdateModule;

impl BasicDrawable {
    pub(super) fn native_laser_updates(&mut self) -> impl Iterator<Item = &mut LaserUpdateModule> {
        self.draw_modules.iter_mut().filter_map(|slot| {
            slot.logic_module_mut()?
                .as_any_mut()
                .downcast_mut::<LaserUpdateModule>()
        })
    }

    pub(crate) fn native_owned_laser_count(&mut self) -> usize {
        self.native_laser_updates()
            .filter(|module| module.uses_owned_services())
            .count()
    }

    pub(crate) fn bind_native_owned_lasers(&mut self, id: u32) {
        for module in self.native_laser_updates() {
            if module.uses_owned_services() {
                module.bind_owned_drawable(id);
            }
        }
    }

    pub(crate) fn with_native_laser<T>(
        &mut self,
        operation: impl FnOnce(&mut LaserUpdateModule) -> Result<T, String>,
    ) -> Result<T, String> {
        let module = self
            .native_laser_updates()
            .next()
            .ok_or_else(|| "actual LaserUpdate module unavailable".to_string())?;
        if !module.uses_owned_services() {
            return Err(
                "classic LaserUpdate cannot receive native owned particle/client services".into(),
            );
        }
        operation(module)
    }

    pub(crate) fn draw_native_laser_geometry(
        &mut self,
        texture_aspect: &dyn Fn(&str) -> Result<f32, String>,
        terrain_height: &mut dyn FnMut(f32, f32) -> Option<f32>,
    ) -> Result<Vec<game_engine::common::system::SceneLineDesc>, String> {
        // C++ Drawable::draw calls getDrawModules before W3DLaserDraw::doDrawModule.
        // This is an explicit Draw phase; CU lookup and identity operations stay inert.
        self.flush_dirty_model_condition();
        let modules = &mut self.draw_modules;
        let update_index = modules
            .iter_mut()
            .position(|slot| {
                slot.logic_module_mut()
                    .is_some_and(|module| module.as_any().is::<LaserUpdateModule>())
            })
            .ok_or_else(|| "actual LaserUpdate unavailable".to_string())?;
        let update = modules[update_index]
            .logic_module_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<LaserUpdateModule>()
            .unwrap();
        if !update.uses_owned_services() {
            return Err("classic LaserUpdate cannot receive native owned draw services".into());
        }
        let mut output = Vec::new();
        for index in 0..modules.len() {
            let Some(draw) = modules[index]
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<W3DLaserDraw>())
            else {
                continue;
            };
            let self_dirty = draw.is_self_dirty();
            let aspect = if draw.texture_name().is_empty() {
                1.0
            } else {
                texture_aspect(draw.texture_name())?
            };
            let update = modules[update_index]
                .logic_module_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<LaserUpdateModule>()
                .unwrap();
            let input = LaserDrawInput::consume(update, self_dirty);
            let draw = modules[index]
                .logic_module_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<W3DLaserDraw>()
                .unwrap();
            if let Some(input) = input {
                draw.geometry_with_services(input, aspect, terrain_height);
            }
            output.extend(draw.current_geometry());
        }
        Ok(output)
    }
}

impl BasicDrawable {
    pub(crate) fn configure_native_tracers(
        &mut self,
        mut configure: impl FnMut(&mut gamelogic::object::draw::W3DTracerDraw),
    ) {
        for slot in &mut self.draw_modules {
            if let Some(module) = slot.logic_module_mut().and_then(|module| {
                module
                    .as_any_mut()
                    .downcast_mut::<gamelogic::object::draw::W3DTracerDraw>()
            }) {
                configure(module);
            }
        }
    }
    pub(crate) fn draw_native_tracer_geometry(
        &mut self,
        logic_frame: u32,
    ) -> Vec<game_engine::common::system::SceneLineDesc> {
        let has_tracer = self.draw_modules.iter_mut().any(|slot| {
            slot.logic_module_mut().is_some_and(|module| {
                module
                    .as_any()
                    .is::<gamelogic::object::draw::W3DTracerDraw>()
            })
        });
        if !has_tracer {
            return Vec::new();
        }
        // C++ getDrawModules flushes pending model conditions before draw.
        self.flush_dirty_model_condition();
        let transform = self.native_base_transform().to_glam();
        let mut lines = Vec::new();
        self.configure_native_tracers(|module| {
            lines.push(module.draw_geometry_at_frame(&transform, logic_frame));
        });
        lines
    }
}
