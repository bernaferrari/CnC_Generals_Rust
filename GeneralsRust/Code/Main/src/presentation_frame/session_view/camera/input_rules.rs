//! Immutable app/map definition owner. Committed Options remain application-owned.
pub(crate) use game_engine::common::ini::ini_game_data::CameraInputRules;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CameraInputPolicy {
    base: CameraInputRules,
    current: CameraInputRules,
    map_parsed_game_data: bool,
}
impl CameraInputPolicy {
    pub(crate) fn for_application(rules: CameraInputRules) -> Self {
        Self {
            base: rules,
            current: rules,
            map_parsed_game_data: false,
        }
    }
    pub(crate) fn current(self) -> CameraInputRules {
        self.current
    }
    pub(crate) fn reset_seed(self) -> Self {
        Self::for_application(self.base)
    }
    pub(crate) fn map_parsed_game_data(self) -> bool {
        self.map_parsed_game_data
    }
    pub(crate) fn clear_map_rules(&mut self) {
        self.map_parsed_game_data = false;
        self.current = self.base;
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn camera_input_rules(&self) -> CameraInputRules {
        self.session_view.camera.input_policy.current()
    }
    pub(crate) fn admit_camera_application_input(&mut self, rules: CameraInputRules) {
        self.session_view.camera.input_policy = CameraInputPolicy::for_application(rules);
    }
    pub(crate) fn apply_camera_input_overrides(&mut self, texts: &[String]) -> Result<(), String> {
        let mut next = self.camera_input_rules();
        let mut parsed = self.session_view.camera.input_policy.map_parsed_game_data;
        for text in texts {
            let (rules, did_parse) = next.overlay_ini_text_with_parse_observation(text)?;
            next = rules;
            parsed |= did_parse;
        }
        self.session_view.camera.input_policy.current = next;
        self.session_view.camera.input_policy.map_parsed_game_data = parsed;
        Ok(())
    }
}
