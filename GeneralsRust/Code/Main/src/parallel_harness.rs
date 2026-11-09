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
        let map_path = crate::game_logic::script_loader::find_map_file(HARNESS_MAP)
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
        }
    }
}

/// Run one instance to completion on the calling thread.
pub fn run_headless_skirmish(
    assets: &SharedSkirmishAssets,
    spec: SkirmishSpec,
) -> Result<SkirmishTrace, String> {
    let mut game = HeadlessSkirmish::start(assets, spec.seed)?;
    let initial = game.observe();
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
