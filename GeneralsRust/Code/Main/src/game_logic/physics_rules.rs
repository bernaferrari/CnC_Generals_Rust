//! Immutable receiving Physics definitions copied into canonical host actors.
pub(crate) use game_engine::common::ini::ini_game_data::PhysicsContentRules;

/// Explicit standalone-fixture defaults: retail GameData.ini31939–31946.
/// Gravity=-64 dist/sec² is parsed to logic-frame acceleration by /900.
pub(crate) fn default_host_physics_rules() -> PhysicsContentRules {
    PhysicsContentRules {
        gravity: -64.0 / 900.0,
        ground_stiffness: 0.8,
        structure_stiffness: 0.3,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PhysicsDefinitions {
    pub(crate) base: PhysicsContentRules,
    pub(crate) effective: PhysicsContentRules,
}
impl Default for PhysicsDefinitions {
    fn default() -> Self {
        let rules = default_host_physics_rules();
        Self {
            base: rules,
            effective: rules,
        }
    }
}
impl super::GameLogic {
    pub(crate) fn admit_physics_rules_base(
        &mut self,
        rules: PhysicsContentRules,
    ) -> Result<(), String> {
        let rules = rules.validate()?;
        if !self.objects.registered_is_empty() {
            return Err("Physics content admission requires an empty receiving world".into());
        }
        let projectile_candidate = self
            .projectile_definitions
            .with_physics_gravity(rules.gravity)?;
        self.physics_definitions = PhysicsDefinitions {
            base: rules,
            effective: rules,
        };
        self.projectile_definitions = projectile_candidate;
        Ok(())
    }
    pub(crate) fn physics_rules_baseline(&self) -> PhysicsContentRules {
        self.physics_definitions.base
    }
    pub(crate) fn admit_physics_map_rules(
        &mut self,
        rules: PhysicsContentRules,
    ) -> Result<(), String> {
        let rules = rules.validate()?;
        if !self.objects.registered_is_empty() {
            return Err("Map Physics admission requires an empty receiving world".into());
        }
        let projectile_candidate = self
            .projectile_definitions
            .with_physics_gravity(rules.gravity)?;
        self.physics_definitions.effective = rules;
        self.projectile_definitions = projectile_candidate;
        Ok(())
    }
}
impl super::Object {
    pub(crate) fn bind_physics_rules(
        &mut self,
        rules: PhysicsContentRules,
        definition_gravity: f32,
    ) {
        self.physics_rules = rules;
        if let Some(height) = self.thing().template.min_fall_height_for_damage {
            self.min_fall_speed_for_damage = (2.0 * definition_gravity * height).abs().sqrt();
        }
    }
    pub(crate) fn physics_gravity(&self) -> f32 {
        self.physics_rules.gravity
    }
    /// CPP PhysicsUpdate485–489: clamp only the bounce consumption.
    /// The immutable authored definition remains unchanged.
    pub(crate) fn physics_ground_stiffness(&self) -> f32 {
        self.physics_rules.ground_stiffness.clamp(0.01, 0.99)
    }
    /// CPP PhysicsUpdate1339–1345 uses the same consumption clamp.
    pub(crate) fn physics_structure_stiffness(&self) -> f32 {
        self.physics_rules.structure_stiffness.clamp(0.01, 0.99)
    }
}

#[cfg(test)]
#[path = "physics_rules_owner_tests.rs"]
mod physics_rules_owner_tests;
