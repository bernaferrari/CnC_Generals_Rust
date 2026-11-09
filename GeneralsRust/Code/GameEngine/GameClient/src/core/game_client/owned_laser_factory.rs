// Shared inert LaserUpdate construction and binding for actual client factories.
impl GameClient {
    fn make_owned_laser_module(
        name_key: game_engine::common::thing::module::NameKeyType,
        data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> GameClientResult<LaserUpdateModule> {
        // The actual registry assigns/binds the ID before any update executes.
        Ok(LaserUpdateModule::new_owned_with_module_data(name_key, data, 0))
    }

    fn owned_laser_count(drawable: &mut dyn Drawable) -> usize {
        let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return 0;
        };
        basic.native_owned_laser_count()
    }

    fn bind_owned_laser_module(drawable: &mut dyn Drawable, id: DrawableId) {
        let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return;
        };
        basic.bind_native_owned_lasers(id.0);
    }
}
