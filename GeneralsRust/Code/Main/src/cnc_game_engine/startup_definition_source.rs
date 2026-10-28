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
    body_auto_particles: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    lighting: game_engine::common::ini::ini_game_data::LightingContentRules,
    static_lod_level: Option<game_engine::common::ini::ini_game_lod::StaticGameLODLevel>,
    dynamic_debris_skip_mask: u32,
    dynamic_debris_generations: u32,
    roads: game_engine::common::ini::ini_road::TerrainRoadCollection,
    physics: game_engine::common::ini::ini_game_data::PhysicsContentRules,
    energy: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
    power_bar: game_engine::common::ini::ini_game_data::PowerBarRules,
    historic_damage_limit: u32,
    sell_percentage: f32,
    max_beacons_per_player: i32,
    combat_bonus: game_engine::common::ini::ini_game_data::CombatBonusRules,
    upgrade_definitions: gamelogic::upgrade::center::UpgradeCenter,
    player_templates: Option<game_engine::common::rts::player_template::PlayerTemplateStore>,
    locomotors: Option<game_engine::common::ini::ini_locomotor::LocomotorStore>,
}
impl StartupSimulationRules {
    pub(super) fn capture(world: &crate::game_logic::GameLogic) -> Self {
        Self {
            body: world.body_damage_baseline(),
            body_auto_particles: world.body_auto_particle_baseline().clone(),
            lighting: world.lighting_rules_baseline().clone(),
            static_lod_level: world.current_static_lod_level(),
            dynamic_debris_skip_mask: world.current_dynamic_debris_skip_mask(),
            dynamic_debris_generations: world.current_dynamic_debris_generations(),
            roads: world.terrain_road_baseline().clone(),
            physics: world.physics_rules_baseline(),
            energy: world.production_energy_baseline(),
            power_bar: world.power_bar_baseline(),
            historic_damage_limit: world.combat_system.historic_damage_limit(),
            sell_percentage: world.session_sell_percentage,
            max_beacons_per_player: world.max_beacons_per_player,
            combat_bonus: world.combat_bonus_baseline().clone(),
            upgrade_definitions: world.upgrade_definitions.clone(),
            player_templates: world.player_template_definitions.clone(),
            locomotors: world.locomotor_definition_base.clone(),
        }
    }
    pub(super) fn upgrade_definitions(&self) -> &gamelogic::upgrade::center::UpgradeCenter {
        &self.upgrade_definitions
    }
    /// Use the catalog loaded by the application that owns this startup.
    pub(super) fn admit_upgrade_definitions(
        &mut self,
        definitions: gamelogic::upgrade::center::UpgradeCenter,
    ) {
        self.upgrade_definitions = definitions;
    }
    pub(super) fn admit_player_template_definitions(
        &mut self,
        definitions: game_engine::common::rts::player_template::PlayerTemplateStore,
    ) {
        self.player_templates = Some(definitions);
    }
    pub(super) fn admit_locomotor_definitions(
        &mut self,
        definitions: game_engine::common::ini::ini_locomotor::LocomotorStore,
    ) {
        self.locomotors = Some(definitions);
    }
    /// Object templates resolve RiderChange locomotor sets during admission.
    /// Transfer this catalog before that admission, retaining the existing
    /// timing of the other startup rules.
    pub(super) fn admit_locomotors_before_objects(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        if let Some(definitions) = self.locomotors.take() {
            world.admit_locomotor_definitions(definitions)?;
        }
        Ok(())
    }
    /// Parse fallback text into this owned recipient using the same Common INI
    /// Upgrade block handler and order, without selecting Core AI.
    pub(super) fn admit_upgrade_ini_text(&mut self, text: &str) -> Result<(), String> {
        crate::assets::ini_template_loader::load_upgrade_ini_text(
            &mut self.upgrade_definitions,
            text,
        )
    }
    pub(super) fn admit(self, world: &mut crate::game_logic::GameLogic) -> Result<(), String> {
        world.admit_body_damage_baseline(self.body);
        world.admit_body_auto_particle_baseline(self.body_auto_particles);
        world.admit_lighting_rules_base(self.lighting);
        world.admit_static_lod_level(self.static_lod_level);
        world.admit_dynamic_debris_runtime_state(
            self.dynamic_debris_skip_mask,
            self.dynamic_debris_generations,
        );
        world.admit_terrain_road_definitions(self.roads)?;
        world.admit_physics_rules_base(self.physics)?;
        world.admit_production_energy_base(self.energy);
        world.admit_power_bar_base(self.power_bar);
        world.admit_historic_damage_limit(self.historic_damage_limit);
        world.admit_sell_percentage(self.sell_percentage);
        world.admit_max_beacons_per_player(self.max_beacons_per_player);
        world.admit_combat_bonus_baseline(self.combat_bonus);
        world.upgrade_definition_base = self.upgrade_definitions.clone();
        world.upgrade_definitions = self.upgrade_definitions;
        if let Some(definitions) = self.locomotors {
            world.admit_locomotor_definitions(definitions)?;
        }
        if let Some(definitions) = self.player_templates {
            world.admit_player_template_definitions(definitions)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn sell_percentage_survives_startup_handoff_and_peer_reset() {
        let rule = game_engine::common::ini::ini_game_data::SellPercentageRules::default()
            .overlay_ini_text("GameData\n SellPercentage = 25%\nEnd\n")
            .unwrap()
            .overlay_ini_text("GameData\n SellPercentage = 40%\nEnd\n")
            .unwrap();
        let mut source = crate::game_logic::GameLogic::new();
        source.admit_sell_percentage(rule.percentage);
        let captured = StartupSimulationRules::capture(&source);
        let mut peer = crate::game_logic::GameLogic::new();
        peer.admit_sell_percentage(0.9);
        peer.reset();
        source.reset();
        drop(source);
        let mut candidate = crate::game_logic::GameLogic::new();
        captured.admit(&mut candidate).unwrap();
        assert_eq!(candidate.session_sell_percentage, 0.4);
        assert_eq!(peer.session_sell_percentage, 0.9);
    }

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
            default_structure_rubble_height: 1.0,
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
