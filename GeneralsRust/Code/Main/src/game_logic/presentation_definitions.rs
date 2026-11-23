//! Application-owned INI definitions for session audio and animations.
use game_engine::common::ini::{
    ini_animation::{Anim2DTemplate, OwnedAnim2DTemplates},
    ini_game_data::PromotionContentRules,
    ini_misc_audio::MiscAudio,
};
#[derive(Debug, Default)]
pub(crate) struct PresentationDefinitions {
    pub(crate) rules: PromotionContentRules,
    misc_audio: MiscAudio,
    animations: OwnedAnim2DTemplates,
}
impl PresentationDefinitions {
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
            admitted.misc_audio = admitted
                .misc_audio
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
        let name = self.misc_audio.unit_promoted.event_name.as_str();
        (!name.is_empty() && !name.eq_ignore_ascii_case("NoSound")).then_some(name)
    }

    /// Player.cpp:3132–3201 selects the receiving MiscAudio radar event.
    pub(crate) fn radar_edge_sound(&self, online: bool) -> Option<&str> {
        let event = if online {
            &self.misc_audio.radar_online_sound
        } else {
            &self.misc_audio.radar_offline_sound
        };
        let name = event.playable_event_name();
        (!name.is_empty()).then_some(name)
    }

    #[cfg(test)]
    pub(crate) fn with_radar_cues(online: &str, offline: &str) -> Self {
        let source = format!(
            "MiscAudio\n RadarNotifyOnlineSound = {online}\n RadarNotifyOfflineSound = {offline}\nEnd\n"
        );
        Self::from_ordered_sources(&[], &[("MiscAudio.ini", source)], &[], &|_| false).unwrap()
    }
    pub(crate) fn salvage_sound(&self, upgrade: bool) -> Option<&str> {
        let event = if upgrade {
            &self.misc_audio.crate_salvage
        } else {
            &self.misc_audio.crate_money
        };
        let name = event.playable_event_name();
        (!name.is_empty()).then_some(name)
    }

    pub(crate) fn level_gain_template(&self) -> Option<&Anim2DTemplate> {
        self.animations.find(&self.rules.animation_name)
    }
    pub(crate) fn animation_templates(&self) -> &OwnedAnim2DTemplates {
        &self.animations
    }
}

impl super::GameLogic {
    pub(crate) fn admit_presentation_definitions(
        &mut self,
        definitions: PresentationDefinitions,
    ) -> Result<(), String> {
        if !self.objects.registered_is_empty() {
            return Err("Presentation definitions require an empty receiving world".into());
        }
        if !definitions.rules.display_seconds.is_finite()
            || !definitions.rules.rise_per_second.is_finite()
        {
            return Err("Non-finite promotion animation definition".into());
        }
        self.session_view.definitions = definitions;
        self.session_view.world_animations.clear();
        Ok(())
    }
}
