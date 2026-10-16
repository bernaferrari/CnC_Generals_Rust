//! W3DView::shake reads current GameData at admission; live impulse state is
//! separate and is never recaptured when application/map definitions change.
use game_engine::common::ini::ini_fx_list::CameraShakeType;
pub(crate) use game_engine::common::ini::ini_game_data::CameraShakeRules;
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraShakePolicy {
    base: CameraShakeRules,
    current: CameraShakeRules,
}
impl Default for CameraShakePolicy {
    fn default() -> Self {
        let rules = CameraShakeRules::default();
        Self {
            base: rules,
            current: rules,
        }
    }
}
impl CameraShakePolicy {
    pub(crate) fn for_application(rules: CameraShakeRules) -> Result<Self, String> {
        let rules = rules.validate()?;
        Ok(Self {
            base: rules,
            current: rules,
        })
    }
    pub(crate) fn validate(self) -> Result<Self, String> {
        self.base.validate()?;
        self.current.validate()?;
        Ok(self)
    }
    pub(crate) fn base(self) -> CameraShakeRules {
        self.base
    }
    pub(crate) fn current(self) -> CameraShakeRules {
        self.current
    }
    pub(crate) fn admit_map(&mut self, rules: CameraShakeRules) -> Result<(), String> {
        self.current = rules.validate()?;
        Ok(())
    }
    pub(crate) fn clear_map_rules(&mut self) {
        self.current = self.base;
    }
    pub(crate) fn reset_seed(mut self) -> Self {
        self.clear_map_rules();
        self
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn admit_camera_application_shake(
        &mut self,
        rules: CameraShakeRules,
    ) -> Result<(), String> {
        self.session_view.camera.shake_policy = CameraShakePolicy::for_application(rules)?;
        Ok(())
    }
    pub(crate) fn admit_camera_map_shake(&mut self, rules: CameraShakeRules) -> Result<(), String> {
        self.session_view.camera.shake_policy.admit_map(rules)
    }
    /// The borrowed script walk owns the actual Logic clock. The approved
    /// deterministic angle algorithm never discovers a presentation Engine.
    pub(crate) fn admit_script_screen_shake(&mut self, intensity: i32) {
        self.enqueue_script_screen_shake_at(intensity, self.get_current_frame() as u32);
    }

    /// ScriptActions.cpp5030 calls the same source shake at current position.
    /// Retain the already-approved deterministic angle seed; only rule owner
    /// and the shared original impulse algorithm change here.
    pub(crate) fn enqueue_script_screen_shake_at(&mut self, intensity: i32, seed_frame: u32) {
        let kind = match intensity.clamp(0, 5) {
            0 => CameraShakeType::Subtle,
            1 => CameraShakeType::Normal,
            2 => CameraShakeType::Strong,
            3 => CameraShakeType::Severe,
            4 => CameraShakeType::CineExtreme,
            _ => CameraShakeType::CineInsane,
        };
        let seed = seed_frame
            .wrapping_mul(1_664_525)
            .wrapping_add((intensity as u32).wrapping_mul(1_013_904_223));
        let angle = (seed as f32 / u32::MAX as f32) * std::f32::consts::TAU;
        let target = self.session_view.target;
        let rules = self.session_view.camera.shake_policy.current();
        self.session_view
            .impulse
            .enqueue_with_rules(target, target, kind, &rules, &mut |_, _| angle);
    }
}
#[cfg(test)]
#[path = "shake_rules_tests.rs"]
mod tests;
