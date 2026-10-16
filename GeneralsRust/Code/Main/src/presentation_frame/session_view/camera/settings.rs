//! Source View-init minimum and explicit setDefaultView maximum, on one owner.
use super::height::HeightRules;
pub(crate) use game_engine::common::ini::ini_game_data::CameraContentRules;
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraSettings {
    base: CameraContentRules,
    current: CameraContentRules,
    captured_min: f32,
    effective_max: f32,
}
impl Default for CameraSettings {
    fn default() -> Self {
        Self::for_application(CameraContentRules::default()).expect("finite camera defaults")
    }
}
impl CameraSettings {
    pub(crate) fn for_application(rules: CameraContentRules) -> Result<Self, String> {
        let rules = rules.validate()?;
        Ok(Self {
            base: rules,
            current: rules,
            captured_min: rules.min_camera_height,
            effective_max: rules.max_camera_height.max(rules.min_camera_height),
        })
    }
    pub(crate) fn validate(self) -> Result<Self, String> {
        self.base.validate()?;
        self.current.validate()?;
        if !self.captured_min.is_finite()
            || !self.effective_max.is_finite()
            || self.effective_max < self.captured_min
            || self.captured_min.to_bits() != self.base.min_camera_height.to_bits()
        {
            return Err("invalid captured camera HAG bounds".into());
        }
        Ok(self)
    }
    pub(crate) fn base(self) -> CameraContentRules {
        self.base
    }
    pub(crate) fn current(self) -> CameraContentRules {
        self.current
    }
    pub(crate) fn bounds(self) -> (f32, f32) {
        (self.captured_min, self.effective_max)
    }
    pub(crate) fn maximum(self) -> f32 {
        self.effective_max
    }
    pub(crate) fn admit_map(&mut self, rules: CameraContentRules) -> Result<(), String> {
        self.current = rules.validate()?;
        Ok(())
    }
    pub(crate) fn capture_default(&mut self, scale: f32) -> Result<(), String> {
        let maximum = self.current.max_camera_height * scale;
        if !scale.is_finite() || !maximum.is_finite() {
            return Err("nonfinite effective camera default maximum".into());
        }
        let mut candidate = *self;
        candidate.effective_max = maximum.max(self.captured_min);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    /// Client/UI reset precedes GlobalData override removal (reverse subsystem order).
    pub(crate) fn reset_seed(mut self) -> Self {
        // Multiplication by one is finite for every admitted content input.
        self.effective_max = self.current.max_camera_height.max(self.captured_min);
        self.current = self.base;
        self
    }
    pub(crate) fn clear_map_rules(&mut self) {
        self.current = self.base;
    }
    pub(crate) fn height_rules(self) -> HeightRules {
        HeightRules {
            min: self.captured_min,
            max: self.effective_max,
            enforce_max: self.current.enforce_max_camera_height,
            adjust_speed: self.current.camera_adjust_speed,
            scroll_cutoff: self.base.scroll_amount_cutoff,
        }
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn admit_camera_application_rules(
        &mut self,
        rules: CameraContentRules,
    ) -> Result<(), String> {
        self.session_view.camera.settings = CameraSettings::for_application(rules)?;
        Ok(())
    }
    pub(crate) fn admit_camera_map_rules(
        &mut self,
        rules: CameraContentRules,
    ) -> Result<(), String> {
        self.session_view.camera.settings.admit_map(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rules(min: f32, max: f32) -> CameraContentRules {
        CameraContentRules {
            min_camera_height: min,
            max_camera_height: max,
            ..Default::default()
        }
    }
    #[test]
    fn map_inputs_do_not_recapture_view_minimum_or_default_maximum() {
        let mut owned = CameraSettings::for_application(rules(100., 300.)).unwrap();
        owned.admit_map(rules(200., 600.)).unwrap();
        assert_eq!(owned.bounds(), (100., 300.));
        owned.capture_default(0.1).unwrap();
        assert_eq!(owned.bounds(), (100., 100.));
        assert_eq!(owned.height_rules().min, 100.);
        assert_eq!(owned.height_rules().max, 100.);
        let cutoff = owned.height_rules().scroll_cutoff;
        let mut changed = owned.current();
        changed.scroll_amount_cutoff = cutoff + 100.;
        owned.admit_map(changed).unwrap();
        assert_eq!(owned.height_rules().scroll_cutoff, cutoff);
        owned.capture_default(1.).unwrap();
        assert_eq!(owned.reset_seed().height_rules().scroll_cutoff, cutoff);
    }
    #[test]
    fn reset_captures_outgoing_map_before_removing_its_override_and_peer_is_unchanged() {
        let mut a = CameraSettings::for_application(rules(100., 300.)).unwrap();
        let b = CameraSettings::for_application(rules(40., 900.)).unwrap();
        a.admit_map(rules(200., 600.)).unwrap();
        a.capture_default(0.5).unwrap();
        let reset = a.reset_seed();
        assert_eq!(reset.bounds(), (100., 600.));
        assert_eq!(reset.current(), rules(100., 300.));
        assert_eq!(b.bounds(), (40., 900.));
        assert_eq!(a.bounds(), (100., 300.));
    }
}

#[cfg(test)]
mod numeric_admission_tests {
    use super::*;
    #[test]
    fn finite_product_overflow_and_impossible_captured_min_reject_without_mutation() {
        let mut settings = CameraSettings::for_application(CameraContentRules {
            min_camera_height: 100.,
            max_camera_height: 300.,
            ..Default::default()
        })
        .unwrap();
        let before = settings;
        assert!(settings.capture_default(f32::MAX).is_err());
        assert_eq!(settings, before);
        let mut impossible = settings;
        impossible.captured_min = 99.;
        assert!(impossible.validate().is_err());
        assert_eq!(settings, before);
    }
}
