// Shared inert LaserUpdate construction and binding for actual client factories.
impl GameClient {
    fn make_owned_laser_module(
        name_key: game_engine::common::thing::module::NameKeyType,
        data: &dyn game_engine::common::thing::module::ModuleData,
    ) -> GameClientResult<LaserUpdateModule> {
        let data = if let Some(data) = data.as_any().downcast_ref::<LaserUpdateModuleData>() {
            data.clone()
        } else if let Some(compatibility) =
            data.as_any()
                .downcast_ref::<gamelogic::object::behavior::laser_update::LaserUpdateModuleData>()
        {
            LaserUpdateModuleData {
                module_tag_name_key: data.get_module_tag_name_key(),
                particle_system_name: compatibility.particle_system_name.clone(),
                target_particle_system_name: compatibility.target_particle_system_name.clone(),
                punch_through_scalar: compatibility.punch_through_scalar,
            }
        } else {
            return Err(GameClientError::InvalidOperation(
                "LaserUpdate module data type mismatch".into(),
            ));
        };
        // The actual registry assigns/binds the ID before any update executes.
        Ok(LaserUpdateModule::new_owned(name_key, Arc::new(data), 0))
    }

    fn owned_laser_count(drawable: &mut dyn Drawable) -> usize {
        let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return 0;
        };
        let mut count = 0;
        for slot in basic.get_draw_modules_mut() {
            if slot
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<LaserUpdateModule>())
                .is_some_and(|module| module.uses_owned_services())
            {
                count += 1;
            }
        }
        count
    }

    fn bind_owned_laser_module(drawable: &mut dyn Drawable, id: DrawableId) {
        let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return;
        };
        for slot in basic.get_draw_modules_mut() {
            if let Some(module) = slot
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<LaserUpdateModule>())
            {
                if module.uses_owned_services() {
                    module.bind_owned_drawable(id.0);
                }
            }
        }
    }
}
