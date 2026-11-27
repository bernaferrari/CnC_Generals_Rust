//! Receiving immutable ActiveBody policy: one classification authority per admitted lifetime.
use generals_sim::object::BodyDamageType;
pub(crate) use game_engine::common::ini::ini_game_data::BodyDamageRules;

pub(crate) use generals_sim::object::body::BodyDamagePolicy;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BodyDamageDefinitions {
    pub(crate) base: BodyDamagePolicy,
    pub(crate) effective: BodyDamagePolicy,
}
impl super::GameLogic {
    /// Admit application DamageFX before any objects are created. Map-specific
    /// definitions are derived separately from this receiving baseline.
    pub fn admit_damage_fx_source(&mut self, source: &str) -> Result<usize, String> {
        if !self.objects.registered_is_empty() {
            return Err("DamageFX content admission requires an empty receiving world".into());
        }
        let mut definitions = self.damage_fx_base.clone();
        let count = definitions
            .apply_source(source)
            .map_err(|error| error.to_string())?;
        self.damage_fx_base = definitions.clone();
        self.damage_fx_definitions = definitions;
        Ok(count)
    }
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
