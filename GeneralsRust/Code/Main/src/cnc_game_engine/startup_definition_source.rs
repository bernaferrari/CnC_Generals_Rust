//! Synchronous startup reads through the application service admitted at boot.
use crate::assets::manager::AssetManager;
use std::sync::Mutex;

pub(super) fn read_optional_text(
    owner: &Mutex<AssetManager>,
    path: &str,
) -> Result<Option<String>, String> {
    // This worker can wait for other application archive work. Contention is
    // never evidence that a required definition is absent. Release the service
    // before decoding or invoking any INI/parser callback.
    let bytes = {
        let mut manager = owner
            .lock()
            .map_err(|_| format!("startup archive owner poisoned while reading '{path}'"))?;
        manager
            .read_optional_definition_source(path)
            .map_err(|error| format!("startup definition '{path}': {error}"))?
    };
    bytes
        .map(|bytes| {
            String::from_utf8(bytes)
                .map_err(|error| format!("startup definition '{path}' is not UTF-8: {error}"))
        })
        .transpose()
}

/// Immutable application baselines already admitted before the worker starts.
/// Map overrides are applied by the receiving candidate after this transfer.
pub(super) struct StartupSimulationRules {
    body: crate::game_logic::body_damage_rules::BodyDamagePolicy,
    lighting: game_engine::common::ini::ini_game_data::LightingContentRules,
    physics: game_engine::common::ini::ini_game_data::PhysicsContentRules,
    energy: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
    upgrade_definitions: gamelogic::upgrade::center::UpgradeCenter,
}
impl StartupSimulationRules {
    pub(super) fn capture(world: &crate::game_logic::GameLogic) -> Self {
        Self {
            body: world.body_damage_baseline(),
            lighting: world.lighting_rules_baseline().clone(),
            physics: world.physics_rules_baseline(),
            energy: world.production_energy_baseline(),
            upgrade_definitions: world.upgrade_definitions.clone(),
        }
    }
    pub(super) fn upgrade_definitions(&self) -> &gamelogic::upgrade::center::UpgradeCenter {
        &self.upgrade_definitions
    }
    /// Seed this startup recipient from the canonical engine-content center
    /// after its init/default upgrades have been installed.
    pub(super) fn refresh_upgrade_definitions_from_engine_lifetime(&mut self) {
        let center = game_engine::common::system::upgrade::process_lifetime_upgrade_center();
        self.upgrade_definitions = center
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
    }
    /// Parse fallback text into this owned recipient using the same Common INI
    /// Upgrade block handler and order, without selecting Core AI.
    pub(super) fn admit_upgrade_ini_text(&mut self, text: &str) -> Result<(), String> {
        let mut ini = game_engine::common::ini::INI::new();
        ini.set_owned_upgrade_center_target(self.upgrade_definitions.clone());
        let result = ini
            .with_inline_source(text, |ini| ini.parse_current_file())
            .map_err(|error| error.to_string());
        if let Some(center) = ini.take_owned_upgrade_center_target() {
            self.upgrade_definitions = center;
        }
        result
    }
    /// Keep Common's engine-lifetime compatibility owner equal to the one
    /// parsed recipient. This is a clone, not a second INI parse.
    pub(super) fn publish_upgrade_definitions_to_engine_lifetime(&self) {
        let center = game_engine::common::system::upgrade::process_lifetime_upgrade_center();
        *center.write().unwrap_or_else(|error| error.into_inner()) =
            self.upgrade_definitions.clone();
    }
    pub(super) fn admit(self, world: &mut crate::game_logic::GameLogic) -> Result<(), String> {
        world.admit_body_damage_baseline(self.body);
        world.admit_lighting_rules_base(self.lighting);
        world.admit_physics_rules_base(self.physics)?;
        world.admit_production_energy_base(self.energy);
        world.upgrade_definition_base = self.upgrade_definitions.clone();
        world.upgrade_definitions = self.upgrade_definitions;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn contended_owner_waits_and_preserves_read_error_instead_of_missing() {
        let owner = Arc::new(Mutex::new(AssetManager::new().unwrap()));
        let held = owner.lock().unwrap();
        let worker_owner = Arc::clone(&owner);
        let (started, attempt) = mpsc::channel();
        let (finished, result) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            finished
                .send(read_optional_text(&worker_owner, "Data/INI/Science.ini"))
                .unwrap();
        });
        attempt.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(held);
        let error = result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err();
        assert!(error.contains("AssetManager not initialized"), "{error}");
        worker.join().unwrap();
        assert!(
            owner.try_lock().is_ok(),
            "read must release its service loan"
        );
    }
    #[test]
    fn startup_candidate_keeps_captured_baselines_after_source_and_peer_reset() {
        use crate::game_logic::{GameLogic, body_damage_rules::BodyDamagePolicy};
        use game_engine::common::ini::ini_game_data::{
            BodyDamageRules, LightingContentRules, PhysicsContentRules, ProductionEnergyRules,
        };
        let mut source = GameLogic::new();
        let body = BodyDamagePolicy::Receiving(BodyDamageRules::from_thresholds(0.9, 0.4).unwrap());
        source.admit_body_damage_baseline(body);
        source.admit_lighting_rules_base(LightingContentRules {
            num_global_lights: 1,
            ..Default::default()
        });
        let physics = PhysicsContentRules {
            gravity: -0.2,
            ground_stiffness: 0.3,
            structure_stiffness: 0.6,
        };
        source.admit_physics_rules_base(physics).unwrap();
        let energy = ProductionEnergyRules {
            max_line_build_objects: 23,
            ..Default::default()
        };
        source.admit_production_energy_base(energy);
        let captured = StartupSimulationRules::capture(&source);
        let mut peer = GameLogic::new();
        peer.admit_lighting_rules_base(LightingContentRules {
            num_global_lights: 2,
            ..Default::default()
        });
        peer.admit_production_energy_base(ProductionEnergyRules {
            max_line_build_objects: 99,
            ..Default::default()
        });
        source.reset();
        peer.reset();
        drop(source);
        let mut candidate = GameLogic::new();
        captured.admit(&mut candidate).unwrap();
        assert_eq!(candidate.body_damage_baseline(), body);
        assert_eq!(candidate.lighting_rules_baseline().num_global_lights, 1);
        assert_eq!(
            candidate.physics_rules_baseline().gravity.to_bits(),
            physics.gravity.to_bits()
        );
        assert_eq!(
            candidate
                .physics_rules_baseline()
                .ground_stiffness
                .to_bits(),
            physics.ground_stiffness.to_bits()
        );
        assert_eq!(
            candidate
                .physics_rules_baseline()
                .structure_stiffness
                .to_bits(),
            physics.structure_stiffness.to_bits()
        );
        assert_eq!(candidate.production_energy_baseline(), energy);
        assert_eq!(peer.production_energy_baseline().max_line_build_objects, 99);
    }
}
