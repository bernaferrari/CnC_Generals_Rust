//! Experimental in-process headless skirmish harness.
//!
//! Definition sources are shared, while each instance owns a `GameLogic` and
//! publishes its services for startup and updates. This does not establish
//! isolation of the remaining ambient engine state. In particular, running
//! the harness concurrently is an isolation probe, not a guarantee.
//!
//! The Rust logic CRC is a partial determinism witness, not the retail C++
//! checksum or a complete representation of game state.

use crate::cnc_game_engine::{StandaloneStartupSources, load_standalone_startup_sources};
use crate::game_logic::GameLogic;
use crate::skirmish_config::{SkirmishMatchConfig, apply_skirmish_config, golden_skirmish_config};
use game_engine::common::ini::INILoadType;
use game_engine::common::ini::ini_game_data::CameraMovementRules;
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::ini::ini_multiplayer::MultiplayerSettings;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Retail map every harness instance plays.
pub const HARNESS_MAP: &str = "Lone Eagle";

/// Immutable retail content read once and shared by all instances.
pub struct SharedSkirmishAssets {
    startup: StandaloneStartupSources,
    map_path: PathBuf,
    game_data: CameraMovementRules,
    player_templates: PlayerTemplateStore,
    locomotors: LocomotorStore,
    multiplayer: Arc<MultiplayerSettings>,
}

/// Why the shared retail content could not be prepared.
#[derive(Debug)]
pub enum HarnessAssetError {
    /// Retail archives or the harness map are not installed.
    Missing(String),
    /// Retail content exists but could not be admitted.
    Invalid(String),
}

impl std::fmt::Display for HarnessAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(what) => write!(f, "retail assets missing: {what}"),
            Self::Invalid(what) => write!(f, "retail assets invalid: {what}"),
        }
    }
}

impl SharedSkirmishAssets {
    /// Read the retail definition sources and resolve [`HARNESS_MAP`].
    pub fn load_retail() -> Result<Self, HarnessAssetError> {
        // Same retail discovery as the Lone Eagle gates: GENERALS_RETAIL_DIR /
        // GENERALS_ASSETS_DIR or a known windows_game install, so the probe
        // also runs from worktrees without a local windows_game.
        let _ = crate::map_frame_scenario::ensure_assets_env_for_tests();
        let map_path = crate::map_frame_scenario::lone_eagle_map_path()
            .map(PathBuf::from)
            .ok_or_else(|| HarnessAssetError::Missing(format!("{HARNESS_MAP}.map")))?;
        // Owned parse scope: these catalogs are built here, then handed to
        // every instance by value; no instance's services observe the parse.
        let _scope = game_engine::common::common_services::CommonServicesScope::new();
        let startup = load_standalone_startup_sources().map_err(HarnessAssetError::Invalid)?;
        for file in ["GameData.ini", "PlayerTemplate.ini", "Locomotor.ini", "Multiplayer.ini"] {
            if startup.definition_texts(file).next().is_none() {
                return Err(HarnessAssetError::Missing(file.into()));
            }
        }
        let invalid = |path: &str, error: &dyn std::fmt::Display| {
            HarnessAssetError::Invalid(format!("{path}: {error}"))
        };

        let mut game_data = CameraMovementRules::default();
        for (path, text) in startup.definition_texts("GameData.ini") {
            game_data = game_data
                .overlay_ini_text(text)
                .map_err(|error| invalid(path, &error))?;
        }

        let mut player_templates = PlayerTemplateStore::new_owned();
        for (path, text) in startup.definition_texts("PlayerTemplate.ini") {
            player_templates
                .apply_source(text, INILoadType::Overwrite)
                .map_err(|error| invalid(path, &error))?;
        }

        let mut locomotors = LocomotorStore::new_owned();
        for (path, text) in startup.definition_texts("Locomotor.ini") {
            locomotors
                .apply_source(text)
                .map_err(|error| invalid(path, &error))?;
        }
        crate::game_logic::locomotor_bootstrap::seed_missing_host_locomotors(&mut locomotors);

        let mut multiplayer = MultiplayerSettings::new();
        for (path, text) in startup.definition_texts("Multiplayer.ini") {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(text, |ini| {
                ini.parse_current_file_into_multiplayer_settings(&mut multiplayer)
            })
            .map_err(|error| invalid(path, &error))?;
        }
        let multiplayer = Arc::new(multiplayer);

        Ok(Self {
            startup,
            map_path,
            game_data,
            player_templates,
            locomotors,
            multiplayer,
        })
    }

    /// Path of the shared retail map.
    pub fn map_path(&self) -> &std::path::Path {
        &self.map_path
    }

    fn match_config(&self, seed: u32) -> SkirmishMatchConfig {
        let mut config = golden_skirmish_config(&self.map_path.to_string_lossy());
        config.random_seed = seed;
        config
    }
}

/// Inputs that identify one instance's match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkirmishSpec {
    /// Unsigned bit pattern passed as C++ GameInfo's seed. The Rust RNG
    /// backend is not the original C++ generator.
    pub seed: u32,
    /// Logic frames to run after map load.
    pub frames: u32,
    /// Issue the scripted human orders ([`HumanOrders`]) for the human slot.
    pub human_orders: bool,
}

/// Real gameplay input for the golden config's human USA slot (player 0),
/// issued through the same `GameLogic::queue_command` path the UI uses:
/// before frame 1 the Command Center queues a Dozer (C++ MSG_QUEUE_UNIT_CREATE)
/// and the starting Dozer is ordered to build a Power Plant beside it (C++
/// MSG_DOZER_CONSTRUCT). The GLA slot is driven by its own skirmish AI.
pub struct HumanOrders;

impl HumanOrders {
    pub const PLAYER: u32 = 0;
    pub const PRODUCED_UNIT: &'static str = "AmericaVehicleDozer";
    pub const STRUCTURE: &'static str = "AmericaPowerPlant";
}

/// A bounded observation of one world, not a complete simulation snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkirmishObservation {
    pub frame: u32,
    pub logic_crc: u32,
    pub object_count: usize,
    pub players: Vec<(u32, u32, usize)>,
    /// Match seed and the Rust RNG backend's current state words.
    pub base_seed: u32,
    pub random_state: [u32; 6],
    /// FNV-1a over every registered object's id, template, owner, quantized
    /// position and health: catches gameplay divergence the CRC does not cover.
    pub world_digest: u64,
}

/// Initial observation and one observation after each logic step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkirmishTrace {
    pub spec: SkirmishSpec,
    pub initial: SkirmishObservation,
    pub frames: Vec<SkirmishObservation>,
}

/// A headless game instance with its own `GameLogic`.
pub struct HeadlessSkirmish {
    logic: GameLogic,
}

impl HeadlessSkirmish {
    /// Admit the shared content into a fresh world and start the skirmish.
    /// Must run on the thread that will drive the instance.
    pub fn start(assets: &SharedSkirmishAssets, seed: u32) -> Result<Self, String> {
        let mut logic = GameLogic::new();
        logic.with_own_services_mut(|logic| -> Result<(), String> {
            logic.admit_camera_application_movement(assets.game_data.clone());
            logic.admit_locomotor_definitions(assets.locomotors.clone())?;
            logic.admit_startup_content(assets.startup.admit()?)?;
            logic.admit_player_template_definitions(assets.player_templates.clone())?;
            logic.multiplayer_definitions = Some(Arc::clone(&assets.multiplayer));
            apply_skirmish_config(logic, &assets.match_config(seed))?;
            if !logic.load_map(&assets.map_path.to_string_lossy()) {
                return Err(format!("load_map failed for {}", assets.map_path.display()));
            }
            Ok(())
        })?;
        Ok(Self { logic })
    }

    /// Advance one logic frame.
    pub fn advance(&mut self) {
        self.logic.with_own_services_mut(GameLogic::update);
    }

    /// Partial Rust logic checksum (not the retail C++ checksum).
    pub fn logic_crc(&self) -> u32 {
        self.logic.logic_crc()
    }

    /// Advance one logic frame and return the logic CRC after it.
    pub fn step(&mut self) -> u32 {
        self.advance();
        self.logic_crc()
    }

    /// `(player id, supplies, live objects owned)` in player-id order.
    pub fn player_summary(&self) -> Vec<(u32, u32, usize)> {
        let mut players: Vec<_> = self.logic.get_players().values().collect();
        players.sort_unstable_by_key(|player| player.id);
        players
            .into_iter()
            .map(|player| {
                let owned = self
                    .logic
                    .host_objects()
                    .registered_values()
                    .filter(|object| object.owner_player_id == Some(player.id))
                    .count();
                (player.id, player.resources.supplies, owned)
            })
            .collect()
    }

    pub fn frame(&self) -> u32 {
        self.logic.get_frame()
    }

    pub fn object_count(&self) -> usize {
        self.logic.host_objects().registered_len()
    }

    /// Observe owned state without advancing the simulation. This intentionally
    /// covers only the listed fields and the partial Rust logic checksum.
    pub fn observe(&self) -> SkirmishObservation {
        SkirmishObservation {
            frame: self.frame(),
            logic_crc: self.logic_crc(),
            object_count: self.object_count(),
            players: self.player_summary(),
            base_seed: self.logic.logic_base_seed,
            random_state: self.logic.logic_random.seed_words(),
            world_digest: self.world_digest(),
        }
    }

    fn world_digest(&self) -> u64 {
        fn mix(hash: &mut u64, bytes: &[u8]) {
            for byte in bytes {
                *hash ^= u64::from(*byte);
                *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        let mut objects: Vec<_> = self.logic.host_objects().registered_values().collect();
        objects.sort_unstable_by_key(|object| object.id.0);
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for object in objects {
            let position = object.get_position();
            mix(&mut hash, &object.id.0.to_le_bytes());
            mix(&mut hash, object.get_template().name.as_bytes());
            mix(&mut hash, &object.owner_player_id.unwrap_or(u32::MAX).to_le_bytes());
            for axis in [position.x, position.y, position.z] {
                mix(&mut hash, &((axis * 16.0).round() as i64).to_le_bytes());
            }
            mix(&mut hash, &((object.health_fraction() * 1024.0).round() as i32).to_le_bytes());
        }
        hash
    }

    /// Live objects of `player` whose template is `name`.
    pub fn owned_ids(&self, player: u32, name: &str) -> Vec<crate::game_logic::ObjectId> {
        let mut ids: Vec<_> = self
            .logic
            .host_objects()
            .registered_values()
            .filter(|object| object.owner_player_id == Some(player))
            .filter(|object| object.get_template().name.eq_ignore_ascii_case(name))
            .map(|object| object.id)
            .collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }

    /// Queue `command_type` for `player` from `selected` (deterministic id/time).
    pub fn issue(
        &mut self,
        player: u32,
        command_id: u32,
        selected: Vec<crate::game_logic::ObjectId>,
        command_type: crate::command_system::CommandType,
    ) {
        self.logic.queue_command(crate::command_system::GameCommand {
            command_type,
            player_id: player,
            command_id,
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            selected_units: selected,
            modifier_keys: Default::default(),
        });
    }

    /// Issue [`HumanOrders`]; errors when the map start lacks the units.
    pub fn issue_human_orders(&mut self) -> Result<(), String> {
        use crate::command_system::CommandType;
        let player = HumanOrders::PLAYER;
        let center = *self
            .owned_ids(player, "AmericaCommandCenter")
            .first()
            .ok_or("human slot has no AmericaCommandCenter")?;
        let dozer = *self
            .owned_ids(player, HumanOrders::PRODUCED_UNIT)
            .first()
            .ok_or("human slot has no starting AmericaVehicleDozer")?;
        let center_at = self
            .logic
            .host_objects()
            .registered_values()
            .find(|object| object.id == center)
            .map(|object| object.get_position())
            .ok_or("command center vanished")?;
        self.issue(player, 1, vec![center], CommandType::QueueUnitCreate {
            template_name: HumanOrders::PRODUCED_UNIT.into(),
            quantity: 1,
        });
        // First legal pad on rings around the Command Center (ground plane is
        // x/z), checked by the same C++ isLocationLegalToBuild residual the
        // placement UI uses.
        let site = (1..=8)
            .flat_map(|ring| {
                let distance = 40.0 + ring as f32 * 15.0;
                (0..12).map(move |step| {
                    let angle = step as f32 * std::f32::consts::TAU / 12.0;
                    glam::Vec3::new(angle.cos() * distance, 0.0, angle.sin() * distance)
                })
            })
            .map(|offset| center_at + offset)
            .find(|site| {
                self.logic.legal_build_code_at_for_builder(
                    crate::game_logic::Team::USA,
                    *site,
                    HumanOrders::STRUCTURE,
                    Some(dozer),
                ) == crate::game_logic::host_production_buildable_command_residual::LBC_OK
            })
            .ok_or("no legal Power Plant pad near the human Command Center")?;
        self.issue(player, 2, vec![dozer], CommandType::DozerConstruct {
            template_name: HumanOrders::STRUCTURE.into(),
            location: site,
            orientation: 0.0,
        });
        Ok(())
    }
}

/// Run one instance to completion on the calling thread.
pub fn run_headless_skirmish(
    assets: &SharedSkirmishAssets,
    spec: SkirmishSpec,
) -> Result<SkirmishTrace, String> {
    let mut game = HeadlessSkirmish::start(assets, spec.seed)?;
    let initial = game.observe();
    if spec.human_orders {
        game.issue_human_orders()?;
    }
    let frames = (0..spec.frames)
        .map(|_| {
            game.advance();
            game.observe()
        })
        .collect();
    Ok(SkirmishTrace { spec, initial, frames })
}

/// Run specs on separate threads; results keep spec order.
/// A panicking instance is reported as an error for that instance only.
pub fn run_parallel_skirmishes(
    assets: &SharedSkirmishAssets,
    specs: &[SkirmishSpec],
) -> Vec<Result<SkirmishTrace, String>> {
    // Hold workers until all spawn attempts have completed. Unlike a barrier,
    // this also releases existing workers if spawning another thread fails.
    let start = std::sync::RwLock::new(());
    let gate = start.write().expect("new start gate");
    std::thread::scope(|scope| {
        let handles: Vec<_> = specs
            .iter()
            .enumerate()
            .map(|(index, &spec)| {
                let start = &start;
                std::thread::Builder::new()
                    .name(format!("skirmish-{index}"))
                    .stack_size(HARNESS_THREAD_STACK)
                    .spawn_scoped(scope, move || {
                        drop(start.read().expect("start gate poisoned"));
                        run_headless_skirmish(assets, spec)
                    })
                    .expect("spawn skirmish thread")
            })
            .collect();
        drop(gate);
        handles
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or_else(|payload| {
                    Err(format!("instance panicked: {}", panic_message(&*payload)))
                })
            })
            .collect()
    })
}

/// Debug builds recurse deeply through map load and AI; match the main thread.
const HARNESS_THREAD_STACK: usize = 64 * 1024 * 1024;

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "<non-string panic>".into())
}
