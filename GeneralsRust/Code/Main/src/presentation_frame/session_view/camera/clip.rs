//! Source lens inputs owned by this receiving session; no second matrix cache.
pub(crate) use game_engine::common::ini::ini_game_data::CameraClipRules;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraClipPolicy {
    base: CameraClipRules,
    current: CameraClipRules,
}
impl CameraClipPolicy {
    pub(crate) fn for_application(rules: CameraClipRules) -> Self {
        Self {
            base: rules,
            current: rules,
        }
    }
    pub(crate) fn current(self) -> CameraClipRules {
        self.current
    }
    pub(crate) fn admit_map(&mut self, rules: CameraClipRules) {
        self.current = rules;
    }
    pub(crate) fn reset_seed(mut self) -> Self {
        self.current = self.base;
        self
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn admit_camera_application_clip(&mut self, rules: CameraClipRules) {
        self.session_view.camera.clip_policy = CameraClipPolicy::for_application(rules);
    }
    pub(crate) fn admit_camera_map_clip(&mut self, rules: CameraClipRules) {
        self.session_view.camera.clip_policy.admit_map(rules);
    }
}
