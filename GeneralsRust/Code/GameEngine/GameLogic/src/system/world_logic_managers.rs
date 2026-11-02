//! Per-world logic subsystem managers.
//!
//! Formerly process `OnceLock` / `Lazy` singletons under `system/` and
//! `commands/command_queue`. Each `WorldServices` owns a fresh bundle so
//! parallel instances never share stealth, detection, beacon, cave, or
//! command-queue state.

use crate::commands::command_queue::CommandQueueManager;
use crate::common::{Int, MAX_PLAYER_COUNT};
use crate::system::beacon_manager::BeaconManager;
use crate::system::cave_system::CaveSystem;
use crate::system::detection_events::DetectionEventManager;
use crate::system::detection_manager::DetectionManager;
use crate::system::detection_modifiers::DetectionModifierCalculator;
use crate::system::disguise_manager::DisguiseManager;
use crate::system::explored_territory::ExploredTerritoryManager;
use crate::system::stealth_conditions::StealthConditionsManager;
use crate::system::stealth_integration::StealthSystemManager;
use crate::system::stealth_integration_layer::{StealthIntegrationLayer, StealthSystemConfig};
use crate::system::stealth_manager::StealthManager;
use crate::system::stealth_special_power::StealthSpecialPowerManager;
use crate::system::stealth_upgrade::StealthUpgradeManager;
use std::sync::{Arc, Mutex};

/// Bundle of GameLogic subsystem managers owned by one world/session.
pub struct WorldLogicManagers {
    pub stealth: Arc<Mutex<StealthManager>>,
    pub detection: Arc<Mutex<DetectionManager>>,
    pub disguise: Arc<Mutex<DisguiseManager>>,
    pub stealth_conditions: Arc<Mutex<StealthConditionsManager>>,
    pub stealth_upgrade: Arc<Mutex<StealthUpgradeManager>>,
    pub stealth_special_power: Arc<Mutex<StealthSpecialPowerManager>>,
    pub detection_events: Arc<Mutex<DetectionEventManager>>,
    pub detection_modifiers: Arc<Mutex<DetectionModifierCalculator>>,
    pub explored_territory: Arc<Mutex<ExploredTerritoryManager>>,
    pub beacon: Arc<Mutex<BeaconManager>>,
    pub cave: Arc<Mutex<CaveSystem>>,
    pub stealth_integration_layer: Arc<Mutex<StealthIntegrationLayer>>,
    pub stealth_integration: Arc<Mutex<StealthSystemManager>>,
    pub command_queue: Arc<Mutex<CommandQueueManager>>,
}

impl Default for WorldLogicManagers {
    fn default() -> Self {
        let stealth = Arc::new(Mutex::new(StealthManager::new()));
        let detection = Arc::new(Mutex::new(DetectionManager::new()));
        let disguise = Arc::new(Mutex::new(DisguiseManager::new()));
        Self {
            stealth: Arc::clone(&stealth),
            detection: Arc::clone(&detection),
            disguise: Arc::clone(&disguise),
            stealth_conditions: Arc::new(Mutex::new(StealthConditionsManager::new())),
            stealth_upgrade: Arc::new(Mutex::new(StealthUpgradeManager::new())),
            stealth_special_power: Arc::new(Mutex::new(StealthSpecialPowerManager::new())),
            detection_events: Arc::new(Mutex::new(DetectionEventManager::new())),
            detection_modifiers: Arc::new(Mutex::new(DetectionModifierCalculator::new())),
            explored_territory: Arc::new(Mutex::new(ExploredTerritoryManager::new())),
            beacon: Arc::new(Mutex::new(BeaconManager::new())),
            cave: Arc::new(Mutex::new(CaveSystem::new())),
            stealth_integration_layer: Arc::new(Mutex::new(StealthIntegrationLayer::new(
                StealthSystemConfig::default(),
            ))),
            // Facade holds the same Arcs as the leaf managers for this world.
            stealth_integration: Arc::new(Mutex::new(StealthSystemManager::from_managers(
                stealth, detection, disguise,
            ))),
            command_queue: Arc::new(Mutex::new(CommandQueueManager::new(
                MAX_PLAYER_COUNT as Int,
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use crate::system::stealth_manager::get_stealth_manager;
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    #[test]
    fn parallel_worlds_keep_independent_logic_managers() {
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for seed in [11u32, 22u32] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let services = new_world_services();
                with_world_services(&services, || {
                    barrier.wait();
                    get_stealth_manager()
                        .lock()
                        .unwrap()
                        .register_object(seed)
                        .unwrap();
                    barrier.wait();
                    let ok = get_stealth_manager()
                        .lock()
                        .unwrap()
                        .get_stealth_status(seed, 0)
                        .is_ok();
                    let other = get_stealth_manager()
                        .lock()
                        .unwrap()
                        .get_stealth_status(if seed == 11 { 22 } else { 11 }, 0)
                        .is_err();
                    results.lock().unwrap().push((seed, ok, other));
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(s, _, _)| *s);
        assert!(got[0].1 && got[0].2);
        assert!(got[1].1 && got[1].2);
    }
}
