//! Immediate Object update effects borrow the driving session's view.
#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn dispatch_object_phase_fx_with_view(
        name: &str,
        object: &crate::game_logic::Object,
        at_object: bool,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) {
        if at_object {
            let pose = Self::observe_host_fx_object(object, player, shroud);
            Self::dispatch_fx_list_at_object_with_view(name, &pose, None, view);
        } else {
            Self::dispatch_fx_list_at_position_with_view(
                name,
                object.get_position(),
                None,
                0.0,
                0.0,
                None,
                view,
            );
        }
    }
}
