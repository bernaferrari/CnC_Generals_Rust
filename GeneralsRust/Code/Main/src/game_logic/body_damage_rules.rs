//! Receiving immutable ActiveBody policy: one classification authority per admitted lifetime.
use super::host_enum_table_residual::HostBodyDamageType;
pub(crate) use game_engine::common::ini::ini_game_data::BodyDamageRules;

/// Standalone compatibility cannot masquerade as admitted application content.
/// This is immutable definition data, not a second writable Body state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum BodyDamagePolicy {
    #[default]
    StandaloneRetail,
    Receiving(BodyDamageRules),
}
impl BodyDamagePolicy {
    pub(crate) fn health_for_state(self, maximum: f32, state: HostBodyDamageType) -> f32 {
        let rules = match self {
            Self::StandaloneRetail => BodyDamageRules::standalone_retail(),
            Self::Receiving(rules) => rules,
        };
        let (damaged, really_damaged) = rules.thresholds();
        let ratio = match state {
            HostBodyDamageType::Pristine => 1.0,
            HostBodyDamageType::Damaged => damaged,
            HostBodyDamageType::ReallyDamaged => really_damaged,
            HostBodyDamageType::Rubble => 0.0,
        };
        // CPP ActiveBody.cpp216–241 setDamageState, including its -1 HP.
        (maximum * ratio - 1.0).max(0.0)
    }
    pub(crate) fn classify(self, health: f32, maximum: f32) -> HostBodyDamageType {
        let rules = match self {
            Self::StandaloneRetail => BodyDamageRules::standalone_retail(),
            Self::Receiving(rules) => rules,
        };
        let (damaged, really_damaged) = rules.thresholds();
        // CPP ActiveBody.cpp81–104: strict comparisons, including IEEE zero/NaN.
        let ratio = health / maximum;
        if ratio > damaged {
            HostBodyDamageType::Pristine
        } else if ratio > really_damaged {
            HostBodyDamageType::Damaged
        } else if ratio > 0.0 {
            HostBodyDamageType::ReallyDamaged
        } else {
            HostBodyDamageType::Rubble
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BodyDamageDefinitions {
    pub(crate) base: BodyDamagePolicy,
    pub(crate) effective: BodyDamagePolicy,
}
impl super::GameLogic {
    pub(crate) fn body_damage_policy(&self) -> BodyDamagePolicy {
        self.body_damage_definitions.effective
    }
    pub(crate) fn body_damage_baseline(&self) -> BodyDamagePolicy {
        self.body_damage_definitions.base
    }
    /// Admission occurs before actors exist; changing a live world's immutable
    /// content cannot silently leave existing Body policies on another revision.
    pub(crate) fn admit_body_damage_baseline(&mut self, policy: BodyDamagePolicy) {
        assert!(
            self.objects.registered_is_empty(),
            "Body content admission requires an empty receiving world"
        );
        self.body_damage_definitions = BodyDamageDefinitions {
            base: policy,
            effective: policy,
        };
    }
    pub(crate) fn admit_body_damage_companion_sources(
        &mut self,
        texts: &[String],
    ) -> Result<(), String> {
        if !self.objects.registered_is_empty() {
            return Err("Body map content admission requires an empty receiving world".into());
        }
        let mut rules = match self.body_damage_definitions.base {
            BodyDamagePolicy::StandaloneRetail => BodyDamageRules::standalone_retail(),
            BodyDamagePolicy::Receiving(rules) => rules,
        };
        for text in texts {
            rules = rules.overlay_ini_text(text)?;
        }
        self.body_damage_definitions.effective = BodyDamagePolicy::Receiving(rules);
        Ok(())
    }
    pub(crate) fn install_validated_body_damage_policy(&mut self, policy: BodyDamagePolicy) {
        assert!(
            self.objects.registered_is_empty(),
            "Body continuation commits before restored actors"
        );
        self.body_damage_definitions.effective = policy;
    }
}

#[cfg(test)]
#[path = "body_damage_rules_tests.rs"]
mod tests;
