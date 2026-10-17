//! Effective receiving GameData policy, not mutable user preference or saved pose.
//! C++ GlobalData defaults/INI -> CLI -> map overrides; Xfer omits this rule.
pub(crate) use game_engine::common::ini::ini_game_data::CameraMovementRules;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CameraMovementPolicy {
    base: CameraMovementRules,
    current: CameraMovementRules,
}
impl CameraMovementPolicy {
    pub(crate) fn for_application(rules: CameraMovementRules) -> Self {
        Self {
            base: rules,
            current: rules,
        }
    }
    pub(crate) fn current(self) -> CameraMovementRules {
        self.current
    }
    pub(crate) fn reset_seed(self) -> Self {
        Self::for_application(self.base)
    }
    pub(crate) fn clear_map_rules(&mut self) {
        self.current = self.base;
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn camera_follow_partition_cell_size(&self) -> f32 {
        self.session_view
            .camera
            .movement_policy
            .current()
            .partition_cell_size
    }
    pub(crate) fn camera_movement_disabled(&self) -> bool {
        self.session_view
            .camera
            .movement_policy
            .current()
            .disable_camera_movement
    }
    pub(crate) fn admit_camera_application_movement(&mut self, rules: CameraMovementRules) {
        self.session_view.camera.movement_policy = CameraMovementPolicy::for_application(rules);
    }
    /// Actual Map.ini -> Solo.ini source texts, borrowed from the existing
    /// pristine-map companion reader. Preflight all before assigning one owner.
    pub(crate) fn apply_camera_movement_overrides(
        &mut self,
        texts: &[String],
    ) -> Result<(), String> {
        let mut rules = self.session_view.camera.movement_policy.current();
        for text in texts {
            rules = rules.overlay_ini_text(text)?;
        }
        self.session_view.camera.movement_policy.current = rules;
        Ok(())
    }
}
#[cfg(test)]
#[path = "movement_owner_tests.rs"]
mod tests;
