//! One ordered, immutable receiving promotion definition admission.
use game_engine::common::ini::{
    ini_animation::{Anim2DTemplate, OwnedAnim2DTemplates},
    ini_game_data::PromotionContentRules,
    ini_misc_audio::MiscAudio,
};
#[derive(Debug, Default)]
pub(crate) struct PromotionDefinitions {
    pub(crate) rules: PromotionContentRules,
    pub(crate) misc: MiscAudio,
    animations: OwnedAnim2DTemplates,
}
impl PromotionDefinitions {
    pub(crate) fn from_ordered_sources(
        game_data: &[(&str, String)],
        misc_audio: &[(&str, String)],
        animations: &[(&str, String)],
        image_exists: &dyn Fn(&str) -> bool,
    ) -> Result<Self, String> {
        let mut admitted = Self::default();
        for (path, text) in game_data {
            admitted.rules = admitted
                .rules
                .overlay_ini_text(text)
                .map_err(|error| format!("{path}: {error}"))?;
        }
        for (path, text) in misc_audio {
            admitted.misc = admitted
                .misc
                .overlay_ini_text_owned(text)
                .map_err(|error| format!("{path}: {error}"))?;
        }
        for (path, text) in animations {
            admitted.animations = admitted
                .animations
                .overlay_ini_text(text, image_exists)
                .map_err(|error| format!("{path}: {error}"))?;
        }
        Ok(admitted)
    }
    pub(crate) fn unit_promoted_name(&self) -> Option<&str> {
        let name = self.misc.unit_promoted.event_name.as_str();
        (!name.is_empty() && !name.eq_ignore_ascii_case("NoSound")).then_some(name)
    }
    pub(crate) fn level_gain_template(&self) -> Option<&Anim2DTemplate> {
        self.animations.find(&self.rules.animation_name)
    }
    pub(crate) fn animation_templates(&self) -> &OwnedAnim2DTemplates {
        &self.animations
    }
}

impl super::GameLogic {
    pub(crate) fn admit_promotion_definitions(
        &mut self,
        definitions: PromotionDefinitions,
    ) -> Result<(), String> {
        if !self.objects.is_empty() {
            return Err("Promotion definitions require an empty receiving world".into());
        }
        if !definitions.rules.display_seconds.is_finite()
            || !definitions.rules.rise_per_second.is_finite()
        {
            return Err("Non-finite promotion animation definition".into());
        }
        self.promotion_definitions = definitions;
        self.session_view.world_animations.clear();
        Ok(())
    }
}
