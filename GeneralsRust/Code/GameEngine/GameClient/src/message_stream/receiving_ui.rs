//! The UI and authoritative logic frame borrowed for one translator operation.
//! No client state or clock is discovered outside the receiving driver.
use crate::core::subsystems::InGameUISubsystem;

pub struct ReceivingUi<'a> {
    emitted_messages: Option<&'a mut Vec<super::GameMessage>>,
    pub(crate) state: &'a mut InGameUISubsystem,
    pub logic_frame: u32,
    pub client_frame: u32,
}
impl<'a> ReceivingUi<'a> {
    pub fn new(state: &'a mut InGameUISubsystem, logic_frame: u32, client_frame: u32) -> Self {
        Self {
            emitted_messages: None,
            state,
            logic_frame,
            client_frame,
        }
    }
    pub fn with_emitter(mut self, emitted: &'a mut Vec<super::GameMessage>) -> Self {
        self.emitted_messages = Some(emitted);
        self
    }
    pub fn emit(&mut self, message: super::GameMessage) {
        self.emitted_messages
            .as_deref_mut()
            .expect("translator requires its receiving emission sink")
            .push(message);
    }
    pub fn dispatch(
        &mut self,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        action: game_engine::common::message_stream::WindowInputDispatch,
    ) -> Option<game_engine::common::message_stream::WindowInputDispatchResult> {
        let emitted = self
            .emitted_messages
            .as_deref_mut()
            .expect("translator requires its receiving emission sink");
        dispatch(action, emitted, self.state)
    }
    pub fn with_emission_sink<R>(
        &mut self,
        emitted: &mut Vec<super::GameMessage>,
        operation: impl FnOnce(&mut ReceivingUi<'_>) -> R,
    ) -> R {
        let mut receiving = ReceivingUi {
            state: &mut *self.state,
            logic_frame: self.logic_frame,
            client_frame: self.client_frame,
            emitted_messages: Some(emitted),
        };
        operation(&mut receiving)
    }
    pub fn create_move_hint(
        &mut self,
        start: crate::message_stream::Coord3D,
        end: crate::message_stream::Coord3D,
        source_id: u32,
    ) {
        self.state
            .create_move_hint(start, end, source_id, self.client_frame);
    }
    pub fn create_attack_hint(
        &mut self,
        start: crate::message_stream::Coord3D,
        end: crate::message_stream::Coord3D,
        source_id: u32,
    ) {
        self.state
            .create_attack_hint(start, end, source_id, self.client_frame);
    }
    pub fn recorder_playback_active(&self) -> bool {
        self.state.recorder_playback_active
    }
    pub fn are_tooltips_disabled(&self) -> bool {
        self.state.tooltips_disabled_at(self.logic_frame)
    }
    pub fn create_mouseover_hint(&mut self, drawable_id: Option<u32>, is_location_hint: bool) {
        self.state.create_mouseover_hint(
            drawable_id,
            is_location_hint,
            self.logic_frame,
            self.client_frame,
        );
    }
    pub fn military_subtitle(
        &mut self,
        label: &str,
        duration_ms: i32,
        manager: &mut crate::gui::window_manager::WindowManager,
        diplomacy: &mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    ) {
        crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
            label, false, manager, diplomacy,
        );
        self.state
            .push_military_subtitle(label, duration_ms, self.logic_frame, manager);
    }
    pub fn display_cant_build_message(&mut self, text: &str) {
        self.state
            .display_cant_build_message(text, self.logic_frame);
    }
    pub fn message(&mut self, text: &str) {
        self.state.message(text, self.logic_frame);
    }
    pub fn set_hint_text(&mut self, text: &str) {
        self.state.set_hint_text(text, self.logic_frame);
    }
}
impl std::ops::Deref for ReceivingUi<'_> {
    type Target = InGameUISubsystem;
    fn deref(&self) -> &Self::Target {
        self.state
    }
}
impl std::ops::DerefMut for ReceivingUi<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state
    }
}
