use game_engine::common::ini::{
    ensure_challenge_generals_loaded, get_challenge_generals,
    GeneralPersona as CommonGeneralPersona,
};

use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ChallengeGenerals.cpp",
    "crate::gui::challenge_generals",
    "Challenge Generals",
    "Carries General's Challenge selection and profile-preview presentation logic.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralPersonaPort {
    pub campaign: String,
    pub display_name: String,
    pub rank: String,
    pub branch: String,
    pub player_template_name: String,
    pub starts_enabled: bool,
}

impl GeneralPersonaPort {
    fn from_common(persona: &CommonGeneralPersona) -> Self {
        Self {
            campaign: persona.get_campaign().to_string(),
            display_name: persona.get_bio_name().to_string(),
            rank: persona.get_bio_rank().to_string(),
            branch: persona.get_bio_branch().to_string(),
            player_template_name: persona.get_player_template_name().to_string(),
            starts_enabled: persona.is_starting_enabled(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChallengeGeneralsPort {
    pub personas: Vec<GeneralPersonaPort>,
    /// UI selection is transient presentation state, never authored persona data.
    selected_campaign: Option<String>,
}

impl ChallengeGeneralsPort {
    /// Build a read-only UI projection of Common's authored ChallengeMode table.
    ///
    /// The standalone GPUI prototype is intentionally excluded from the default
    /// workspace, but it must not grow a second table of fictional defaults.  If
    /// Common cannot load the retail/mod-authored source, return an empty model so
    /// the UI fails closed instead of presenting selectable fake generals.
    pub fn from_common() -> Self {
        if ensure_challenge_generals_loaded().is_err() {
            return Self::default();
        }

        let store = get_challenge_generals();
        Self {
            personas: store
                .positions
                .iter()
                .map(GeneralPersonaPort::from_common)
                .collect(),
            selected_campaign: None,
        }
    }

    /// Compatibility constructor used by the prototype scene.
    ///
    /// The old implementation manufactured three hard-coded records here. Keep
    /// the call site stable while routing it through the Common-owned loader.
    pub fn init_defaults() -> Self {
        Self::from_common()
    }

    pub fn selected_campaign(&self) -> Option<&str> {
        self.selected_campaign.as_deref()
    }

    pub fn selected_persona(&self) -> Option<&GeneralPersonaPort> {
        self.selected_campaign
            .as_deref()
            .and_then(|campaign| self.get_player_general_by_campaign_name(campaign))
    }

    /// Select a persona without mutating the Common-authored projection.
    pub fn select_campaign(&mut self, campaign: &str) -> bool {
        let selected = self
            .get_player_general_by_campaign_name(campaign)
            .map(|persona| persona.campaign.clone());
        let Some(selected) = selected else {
            return false;
        };

        self.selected_campaign = Some(selected);
        true
    }

    pub fn clear_selection(&mut self) {
        self.selected_campaign = None;
    }

    pub fn get_player_general_by_campaign_name(&self, name: &str) -> Option<&GeneralPersonaPort> {
        self.personas
            .iter()
            .find(|persona| persona.campaign.eq_ignore_ascii_case(name))
    }

    pub fn get_general_by_general_name(&self, name: &str) -> Option<&GeneralPersonaPort> {
        self.personas
            .iter()
            .find(|persona| persona.display_name.eq_ignore_ascii_case(name))
    }

    pub fn get_general_by_template_name(&self, name: &str) -> Option<&GeneralPersonaPort> {
        self.personas
            .iter()
            .find(|persona| persona.player_template_name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_common_persona_fields_without_fabricating_defaults() {
        let mut authored = CommonGeneralPersona::new();
        authored.campaign = "CHALLENGE_AUTHORED".to_string();
        authored.bio_name = "GUI:AuthoredName".to_string();
        authored.bio_rank = "GUI:AuthoredRank".to_string();
        authored.bio_branch = "GUI:AuthoredBranch".to_string();
        authored.player_template_name = "FactionAuthoredGeneral".to_string();
        authored.starts_enabled = true;

        let persona = GeneralPersonaPort::from_common(&authored);
        assert_eq!(persona.campaign, "CHALLENGE_AUTHORED");
        assert_eq!(persona.display_name, "GUI:AuthoredName");
        assert_eq!(persona.rank, "GUI:AuthoredRank");
        assert_eq!(persona.branch, "GUI:AuthoredBranch");
        assert_eq!(persona.player_template_name, "FactionAuthoredGeneral");
        assert!(persona.starts_enabled);
    }

    #[test]
    fn finds_general_by_campaign_name_case_insensitively() {
        let challenge = ChallengeGeneralsPort {
            personas: vec![GeneralPersonaPort {
                campaign: "CHALLENGE_AUTHORED".to_string(),
                display_name: "GUI:AuthoredName".to_string(),
                rank: String::new(),
                branch: String::new(),
                player_template_name: "FactionAuthoredGeneral".to_string(),
                starts_enabled: true,
            }],
            selected_campaign: None,
        };
        let persona = challenge
            .get_player_general_by_campaign_name("challenge_authored")
            .expect("expected persona");

        assert_eq!(persona.display_name, "GUI:AuthoredName");
    }

    #[test]
    fn finds_general_by_template_name() {
        let challenge = ChallengeGeneralsPort {
            personas: vec![GeneralPersonaPort {
                campaign: "CHALLENGE_AUTHORED".to_string(),
                display_name: "GUI:AuthoredName".to_string(),
                rank: String::new(),
                branch: String::new(),
                player_template_name: "FactionAuthoredGeneral".to_string(),
                starts_enabled: true,
            }],
            selected_campaign: None,
        };
        let persona = challenge
            .get_general_by_template_name("FactionAuthoredGeneral")
            .expect("expected persona");

        assert_eq!(persona.display_name, "GUI:AuthoredName");
    }

    #[test]
    fn selection_state_is_separate_from_common_persona_projection() {
        let mut challenge = ChallengeGeneralsPort {
            personas: vec![GeneralPersonaPort {
                campaign: "CHALLENGE_AUTHORED".to_string(),
                display_name: "GUI:AuthoredName".to_string(),
                rank: String::new(),
                branch: String::new(),
                player_template_name: "FactionAuthoredGeneral".to_string(),
                starts_enabled: true,
            }],
            selected_campaign: None,
        };

        assert!(challenge.select_campaign("challenge_authored"));
        assert_eq!(challenge.selected_campaign(), Some("CHALLENGE_AUTHORED"));
        assert_eq!(
            challenge
                .selected_persona()
                .map(|persona| persona.display_name.as_str()),
            Some("GUI:AuthoredName")
        );
        challenge.clear_selection();
        assert!(challenge.selected_persona().is_none());
    }

    #[test]
    fn default_projection_fails_closed_without_authored_data() {
        let challenge = ChallengeGeneralsPort::default();
        assert!(challenge.personas.is_empty());
        assert!(challenge.selected_campaign().is_none());
    }
}
