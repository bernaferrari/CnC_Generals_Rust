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
use game_engine::common::ini::ini_game_data::{CameraMovementRules, ProductionEnergyRules};
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::ini::ini_multiplayer::MultiplayerSettings;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Retail map every harness instance plays.
pub const HARNESS_MAP: &str = "Lone Eagle";

/// Immutable retail content read once and shared by all instances.
pub struct SharedSkirmishAssets {
    definitions: RetailWorldDefinitions,
    map_path: PathBuf,
}

/// Everything C++ GameEngine::init loads before a game can start (GlobalData,
/// PlayerTemplate, Locomotor, Multiplayer, Armor, Terrain, Roads, OCL and the
/// ordered Object INI), read once and admitted into each receiving world in
/// the application boot order.
pub struct RetailWorldDefinitions {
    startup: StandaloneStartupSources,
    game_data: CameraMovementRules,
    /// GameData.ini Low/Min/MaxLowEnergy* rules (ThingTemplate::calcTimeToBuild).
    energy: ProductionEnergyRules,
    /// The remaining simulation baselines the application boot admits.
    rules: RetailLogicRules,
    player_templates: PlayerTemplateStore,
    locomotors: LocomotorStore,
    multiplayer: Arc<MultiplayerSettings>,
}

/// The GameData / definition baselines `CnCGameEngine` boot admits into the
/// live world (C++ GameEngine::init: GlobalData, Armor, Terrain, Roads,
/// ObjectCreationList), read once from the shared retail sources.
#[derive(Clone)]
struct RetailLogicRules {
    physics: game_engine::common::ini::ini_game_data::PhysicsContentRules,
    power_bar: game_engine::common::ini::ini_game_data::PowerBarRules,
    body_damage: game_engine::common::ini::ini_game_data::BodyDamageRules,
    body_auto_particles: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    combat_bonus: game_engine::common::ini::ini_game_data::CombatBonusRules,
    tunnel_capacity: i32,
    historic_damage_limit: u32,
    sell_percentage: f32,
    armor: Arc<gamelogic::object::armor::ArmorDefinitions>,
    terrain: Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
    roads: game_engine::common::ini::ini_road::TerrainRoadCollection,
    object_creation_lists: Vec<String>,
}

impl RetailLogicRules {
    fn read(startup: &StandaloneStartupSources) -> Result<Self, HarnessAssetError> {
        use game_engine::common::ini::ini_game_data as gd;
        fn fold<R>(
            startup: &StandaloneStartupSources,
            init: R,
            overlay: impl Fn(R, &str) -> Result<R, String>,
        ) -> Result<R, HarnessAssetError> {
            startup
                .definition_texts("GameData.ini")
                .try_fold(init, |rules, (path, text)| {
                    overlay(rules, text)
                        .map_err(|error| HarnessAssetError::Invalid(format!("{path}: {error}")))
                })
        }
        let owned = |file: &'static str| -> Vec<(&'static str, String)> {
            startup
                .definition_texts(file)
                .map(|(path, text)| (path, text.to_owned()))
                .collect()
        };
        let invalid = HarnessAssetError::Invalid;
        let physics = fold(
            startup,
            gd::PhysicsContentRules::from_global_data(
                &game_engine::common::global_data::GlobalData::default(),
            ),
            |r, t| r.overlay_ini_text(t),
        )?
        .validate()
        .map_err(invalid)?;
        let armor_texts: Vec<_> = startup.definition_texts("Armor.ini").collect();
        Ok(Self {
            physics,
            power_bar: fold(startup, gd::PowerBarRules::default(), |r, t| r.overlay_ini_text(t))?,
            body_damage: fold(startup, gd::BodyDamageRules::default(), |r, t| r.overlay_ini_text(t))?,
            body_auto_particles: fold(startup, gd::BodyAutoParticleRules::default(), |r, t| {
                r.overlay_ini_text(t).map_err(|e| e.to_string())
            })?,
            combat_bonus: fold(startup, gd::CombatBonusRules::default(), |r, t| r.overlay_ini_text(t))?,
            tunnel_capacity: fold(startup, gd::TunnelCapacityRules::default(), |r, t| {
                r.overlay_ini_text(t).map_err(|e| e.to_string())
            })?
            .max_capacity,
            historic_damage_limit: fold(startup, gd::HistoricDamageLimitRules::default(), |r, t| {
                r.overlay_ini_text(t).map_err(|e| e.to_string())
            })?
            .frames,
            sell_percentage: fold(startup, gd::SellPercentageRules::default(), |r, t| {
                r.overlay_ini_text(t).map_err(|e| e.to_string())
            })?
            .percentage,
            armor: crate::cnc_game_engine::armor_definitions_from_texts(&armor_texts).map_err(invalid)?,
            terrain: crate::game_logic::terrain_definitions::parse_application_terrain_sources(&owned("Terrain.ini"))
                .map_err(invalid)?,
            roads: crate::game_logic::terrain_definitions::parse_application_road_sources(&owned("Roads.ini"))
                .map_err(invalid)?,
            object_creation_lists: owned("ObjectCreationList.ini").into_iter().map(|(_, text)| text).collect(),
        })
    }

    fn admit(&self, logic: &mut GameLogic) -> Result<(), String> {
        logic.admit_combat_bonus_baseline(self.combat_bonus.clone());
        logic.admit_tunnel_capacity_baseline(self.tunnel_capacity);
        logic.admit_historic_damage_limit(self.historic_damage_limit);
        logic.admit_sell_percentage(self.sell_percentage);
        logic.admit_armor_definitions(Arc::clone(&self.armor));
        logic.admit_terrain_definitions(Arc::clone(&self.terrain))?;
        logic.admit_terrain_road_definitions(self.roads.clone())?;
        logic.admit_ocl_application_sources(&self.object_creation_lists)?;
        logic.admit_body_damage_baseline(
            crate::game_logic::body_damage_rules::BodyDamagePolicy::Receiving(self.body_damage),
        );
        logic.admit_physics_rules_base(self.physics)?;
        logic.admit_power_bar_base(self.power_bar);
        logic.admit_body_auto_particle_baseline(self.body_auto_particles.clone());
        Ok(())
    }
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
        Ok(Self {
            definitions: RetailWorldDefinitions::load()?,
            map_path,
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

impl RetailWorldDefinitions {
    /// Read the retail definition sources the application boot admits.
    pub fn load() -> Result<Self, HarnessAssetError> {
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
        let mut energy = ProductionEnergyRules::default();
        for (path, text) in startup.definition_texts("GameData.ini") {
            game_data = game_data
                .overlay_ini_text(text)
                .map_err(|error| invalid(path, &error))?;
            energy = energy
                .overlay_ini_text(text)
                .map_err(|error| invalid(path, &error))?;
        }

        let rules = RetailLogicRules::read(&startup)?;

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
            game_data,
            energy,
            rules,
            player_templates,
            locomotors,
            multiplayer,
        })
    }

    /// Admit these definitions into a fresh world in CnCGameEngine boot
    /// order, before NewGame (C++ GameEngine::init, GameEngine.cpp:428-458).
    pub fn admit(&self, logic: &mut GameLogic) -> Result<(), String> {
        logic.admit_camera_application_movement(self.game_data.clone());
        // The application boot admits GameData's energy rules before any
        // build; without them every calcTimeToBuild hits the 0.01 floor.
        logic.admit_production_energy_base(self.energy);
        // Boot order (boot.rs): GameData baselines, Armor, Terrain,
        // Roads and OCL before any object content is admitted.
        self.rules.admit(logic)?;
        logic.admit_locomotor_definitions(self.locomotors.clone())?;
        logic.admit_startup_content(self.startup.admit()?)?;
        logic.admit_player_template_definitions(self.player_templates.clone())?;
        logic.multiplayer_definitions = Some(Arc::clone(&self.multiplayer));
        Ok(())
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
            assets.definitions.admit(logic)?;
            apply_skirmish_config(logic, &assets.match_config(seed))?;
            if !logic.load_map(&assets.map_path.to_string_lossy()) {
                return Err(format!("load_map failed for {}", assets.map_path.display()));
            }
            Ok(())
        })?;
        let mut game = Self { logic };
        // Hand out the world after its first logic frame, when a player first
        // has control: C++ DozerAIUpdate::getMostRecentCommand only selects a
        // task whose m_taskOrderFrame is above 0, so a frame-0 order is inert.
        game.advance();
        Ok(game)
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

    /// Dozer approach inputs: position, the build task's ACTION dock point
    /// (C++ `m_dockPoint[DOZER_TASK_BUILD][DOZER_DOCK_POINT_ACTION]`), the
    /// locomotor's pristine max speed in units per second and the ground
    /// length still to drive along its current path (None without a path).
    pub fn dozer_approach(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<(glam::Vec3, Option<glam::Vec3>, f32, Option<f32>)> {
        let object = self.logic.host_objects().lookup(&id)?;
        let action = &object.dozer_tasks.docks[0][1];
        let dock = action.valid.then(|| glam::Vec3::new(action.location.x, action.location.y, action.location.z));
        let position = object.get_position();
        let path = &object.movement.path;
        let remaining = (object.movement.current_path_index < path.len()).then(|| {
            let ground = |a: glam::Vec3, b: glam::Vec3| glam::Vec2::new(b.x - a.x, b.z - a.z).length();
            let mut at = position;
            let mut length = 0.0;
            for &node in &path[object.movement.current_path_index..] {
                length += ground(at, node);
                at = node;
            }
            length
        });
        Some((position, dock, object.movement.max_speed, remaining))
    }

    /// `true` when `point` (ground plane x/z) lies inside `id`'s footprint.
    pub fn footprint_contains(&self, id: crate::game_logic::ObjectId, point: glam::Vec3) -> Option<bool> {
        let object = self.logic.host_objects().lookup(&id)?;
        let center = object.get_position();
        let shape = object.geometry_info();
        let (dx, dz) = (point.x - center.x, point.z - center.z);
        Some(match shape.geom_type {
            crate::game_logic::HostGeometryType::Box => {
                let (sin, cos) = object.get_orientation().sin_cos();
                let along = dx * cos + dz * sin;
                let across = -dx * sin + dz * cos;
                along.abs() < shape.major_radius && across.abs() < shape.minor_radius
            }
            _ => dx * dx + dz * dz < shape.major_radius * shape.major_radius,
        })
    }

    /// Boot-admitted rules this world does not hold at their shared retail
    /// value (GameData physics/power bar/sell/tunnel, Armor, Terrain).
    pub fn retail_rule_mismatches(&self, assets: &SharedSkirmishAssets) -> Vec<&'static str> {
        let rules = &assets.definitions.rules;
        let logic = &self.logic;
        let mut wrong = Vec::new();
        if logic.physics_rules_baseline() != rules.physics {
            wrong.push("physics");
        }
        if logic.power_bar_baseline() != rules.power_bar {
            wrong.push("power bar");
        }
        if logic.session_sell_percentage != rules.sell_percentage {
            wrong.push("sell percentage");
        }
        if logic.effective_tunnel_capacity() != usize::try_from(rules.tunnel_capacity).unwrap_or(0) {
            wrong.push("tunnel capacity");
        }
        // A map's Map.ini may overlay rows (map_settings), never drop them.
        if logic.current_armor_definitions().map_or(0, |armor| armor.len()) < rules.armor.len() {
            wrong.push("armor");
        }
        if rules.armor.is_empty() {
            wrong.push("retail armor catalog empty");
        }
        if rules.object_creation_lists.is_empty() {
            wrong.push("retail ObjectCreationList missing");
        }
        wrong
    }

    /// `(id, under construction, construction fraction)` of `player`'s `name` objects.
    pub fn construction(&self, player: u32, name: &str) -> Vec<(crate::game_logic::ObjectId, bool, f32)> {
        let mut sites: Vec<_> = self
            .logic
            .host_objects()
            .registered_values()
            .filter(|object| object.owner_player_id == Some(player))
            .filter(|object| object.get_template().name.eq_ignore_ascii_case(name))
            .map(|object| (object.id, object.status.under_construction, object.construction_percent))
            .collect();
        sites.sort_unstable_by_key(|site| site.0.0);
        sites
    }

    /// Authored `BuildTime` seconds of the loaded `name` template.
    pub fn template_build_seconds(&self, name: &str) -> Option<f32> {
        self.logic.templates.get(name).map(|template| template.build_time)
    }

    /// GameData `(LowEnergyPenaltyModifier, MinLowEnergyProductionSpeed,
    /// MaxLowEnergyProductionSpeed)` admitted to this world.
    pub fn energy_rules(&self) -> (f32, f32, f32) {
        let rules = self.logic.production_energy_rules();
        (rules.penalty, rules.min_speed, rules.max_speed)
    }

    /// C++ `Energy::getEnergySupplyRatio` of `player` now.
    pub fn energy_supply_ratio(&self, player: u32) -> Option<f32> {
        let frame = self.frame();
        self.logic.get_players().get(&player).map(|p| p.energy_supply_ratio_at(frame))
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
        if self.frame() == 0 {
            return Err("orders issued on logic frame 0 are never selected by DozerAIUpdate".into());
        }
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
