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

#[derive(Clone, Debug, Default)]
pub struct ChallengeGeneralsPort {
    pub personas: Vec<GeneralPersonaPort>,
}

impl ChallengeGeneralsPort {
    pub fn init_defaults() -> Self {
        Self {
            personas: vec![
                GeneralPersonaPort {
                    campaign: "BossGeneral".to_string(),
                    display_name: "General Alexander".to_string(),
                    rank: "4 Star General".to_string(),
                    branch: "USA Superweapon".to_string(),
                    player_template_name: "FactionAmericaSuperWeaponGeneral".to_string(),
                    starts_enabled: true,
                },
                GeneralPersonaPort {
                    campaign: "TankGeneral".to_string(),
                    display_name: "General Kwai".to_string(),
                    rank: "General".to_string(),
                    branch: "China Tank".to_string(),
                    player_template_name: "FactionChinaTankGeneral".to_string(),
                    starts_enabled: true,
                },
                GeneralPersonaPort {
                    campaign: "StealthGeneral".to_string(),
                    display_name: "Prince Kassad".to_string(),
                    rank: "Commander".to_string(),
                    branch: "GLA Stealth".to_string(),
                    player_template_name: "FactionGLAStealthGeneral".to_string(),
                    starts_enabled: false,
                },
            ],
        }
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
    fn finds_general_by_campaign_name_case_insensitively() {
        let challenge = ChallengeGeneralsPort::init_defaults();
        let persona = challenge
            .get_player_general_by_campaign_name("bossgeneral")
            .expect("expected persona");

        assert_eq!(persona.display_name, "General Alexander");
    }

    #[test]
    fn finds_general_by_template_name() {
        let challenge = ChallengeGeneralsPort::init_defaults();
        let persona = challenge
            .get_general_by_template_name("FactionChinaTankGeneral")
            .expect("expected persona");

        assert_eq!(persona.display_name, "General Kwai");
    }
}
