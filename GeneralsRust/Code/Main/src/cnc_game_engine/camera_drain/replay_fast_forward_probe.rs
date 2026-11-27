use super::replay_logic_step_count;
use crate::cnc_game_engine::CnCGameEngine;
use crate::command_line::CommandLineArgs;
use crate::game_logic::{
    BuildingData, BuildingType, GameLogic, KindOf, ObjectId, ProductionItem, ProductionKind,
    Resources, Team, ThingTemplate,
};
use glam::Vec3;
use std::sync::Arc;
use winit::{event_loop::EventLoop, window::WindowAttributes};

fn replay_fixture_engine() -> anyhow::Result<(
    EventLoop<()>,
    crate::subsystem_manager::ApplicationServices,
    CnCGameEngine,
)> {
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        event_loop.create_window(
            WindowAttributes::default()
                .with_title("replay fast-forward regression")
                .with_visible(false),
        )?,
    );
    let private_root = std::path::PathBuf::from(
        std::env::var_os("GENERALS_REPLAY_PROBE_PRIVATE_ROOT")
            .ok_or_else(|| anyhow::anyhow!("probe requires private launch root"))?,
    );
    // The probe does not run Engine startup. Admit the captured startup
    // GameData bytes into this receiving owner before any mini-world/map.
    use game_engine::common::ini::ini_game_data::{CameraContentRules, CameraMovementRules};
    let mut movement = CameraMovementRules::default();
    let mut camera = CameraContentRules::default();
    for relative in [
        "Data/INI/Default/GameData.ini",
        "Data/INI/GameData.ini",
        "Data/INI/GameDataDebug.ini",
    ] {
        let path = private_root.join("cwd").join(relative);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(anyhow::anyhow!("{}: {error}", path.display())),
        };
        movement = movement
            .overlay_ini_text(&text)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        camera = camera
            .overlay_ini_text(&text)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
    }
    let args = CommandLineArgs::parse_from_args(vec![
        "replay-fast-forward-test".to_string(),
        "--runtime_host=headless".to_string(),
        "--noaudio".to_string(),
        format!("--userdata={}", private_root.join("userdata").display()),
    ])?;
    let mut application_services = crate::subsystem_manager::ApplicationServices::new();
    let mut engine = pollster::block_on(CnCGameEngine::new(
        window,
        Arc::new(args),
        &mut application_services,
    ))?;
    engine
        .game_logic
        .admit_camera_application_movement(movement);
    engine
        .game_logic
        .admit_camera_application_rules(camera)
        .map_err(anyhow::Error::msg)?;
    Ok((event_loop, application_services, engine))
}

fn replay_factory_template() -> ThingTemplate {
    let mut factory = ThingTemplate::new("ReplayFastForwardFactory");
    factory.set_health(500.0);
    factory.add_kind_of(KindOf::Structure);
    factory.add_kind_of(KindOf::FSBarracks);
    // QueueProductionExitUpdate.cpp:196-206 advances integer delay only
    // for an Object that actually declares this update module.
    factory.production_exit_metadata = Some(crate::game_logic::ProductionExitMetadata {
        style: crate::game_logic::ProductionExitStyle::Queue,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    factory
}

fn install_replay_factory(engine: &mut CnCGameEngine) -> ObjectId {
    let movement = engine
        .game_logic
        .session_view
        .camera
        .movement_policy
        .reset_seed();
    let camera = engine.game_logic.session_view.camera.settings.reset_seed();
    engine.game_logic = GameLogic::new();
    engine.game_logic.session_view.camera.movement_policy = movement;
    // The synthetic world replaces the camera owner after application startup.
    // Keep the admitted application basis so ClearGameData can reset the view.
    engine.game_logic.session_view.camera.settings = camera;
    engine.last_frame_timing = None;
    engine.last_presentation_frame = None;
    engine.host_match_time_frozen = None;
    engine.game_paused = false;

    let factory = replay_factory_template();
    engine
        .game_logic
        .templates
        .insert("ReplayFastForwardFactory".to_string(), factory);
    let producer_id = engine
        .game_logic
        .create_object(
            "ReplayFastForwardFactory",
            Team::USA,
            Vec3::new(8.0, 0.0, 8.0),
        )
        .expect("producer");
    let producer = engine
        .game_logic
        .host_object_mut(producer_id)
        .expect("producer object");
    let mut building = BuildingData::new(BuildingType::Barracks);
    building.production_queue.push(ProductionItem {
        runtime_entry_identity: 0,
        template_name: "ReplayFastForwardRanger".to_string(),
        progress: 0.0,
        total_time: 10.0,
        construction_frames: 0,
        cost: Resources {
            supplies: 100,
            power: 0,
        },
        quantity_total: 1,
        quantity_produced: 0,
        kind: ProductionKind::Unit,
    });
    producer.production_exit_state.delay_frames = 9;
    producer.production_exit_state.initialized = true;
    producer.building_data = Some(building);
    producer_id
}

fn host_queue_state(engine: &CnCGameEngine, producer_id: ObjectId) -> (u32, u32) {
    let producer = engine.game_logic.host_object(producer_id).expect("host producer");
    let building = producer.building_data.as_ref().expect("host producer queue");
    let head = building.production_queue.first().expect("queue head");
    (
        head.construction_frames,
        producer.production_exit_state.delay_frames,
    )
}

fn direct_probe_frame(engine: &mut CnCGameEngine, dt: f32) -> Result<(), String> {
    #[cfg(feature = "game_client")]
    engine.native_display.require_shell_slot()?;
    engine.host_run_ingame_logic_presentation_frame(dt)?;
    engine.complete_native_display_step()?;
    #[cfg(feature = "game_client")]
    engine.native_display.require_shell_slot()?;
    if engine.should_present_w3d_frame() {
        engine.render().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn run_engine_replay_batch(
    engine: &mut CnCGameEngine,
    replay_fast_forward: bool,
    dt: f32,
    outer_frames: usize,
) -> (u32, u32, u32) {
    engine.replay_fast_forward = replay_fast_forward;
    let producer_id = install_replay_factory(engine);
    let before = host_queue_state(engine, producer_id);
    for _ in 0..outer_frames {
        // Exercise the actual engine boundary: it decides fixed-step count,
        // completes Main gameplay and finalizes one
        // presentation frame per outer update.
        direct_probe_frame(engine, dt).expect("complete replay probe frame");
    }
    let after = host_queue_state(engine, producer_id);
    let frame = engine.last_presentation_frame.as_ref().expect("published frame");
    assert_eq!(frame.frame.0, engine.game_logic.get_frame());
    let producer = frame
        .objects
        .iter()
        .find(|object| object.id == producer_id)
        .expect("producer in published roster");
    let live_queue = &engine
        .game_logic
        .host_object(producer_id)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(
        producer.production_queue[0],
        crate::presentation_frame::PresentationProductionItem::from_host_item(&live_queue[0])
    );
    (
        after.0 - before.0,
        before.1 - after.1,
        engine.game_logic.get_frame(),
    )
}

#[cfg(feature = "game_client")]
fn source_command_phase_engine_witness(engine: &mut CnCGameEngine) {
    use crate::cnc_game_engine::GameState;
    use crate::game_logic::GameMode;
    use game_engine::common::message_stream::{
        ClientShellMapLaunch, ClientSkirmishLaunch, GameMessage, GameMessageType,
    };
    let directory = std::env::temp_dir().join(format!(
        "generals-command-phase-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("owned source command inputs");
    let inputs = game_engine::common::recorder::RecorderOwnerInputs {
        map: "Maps/SourcePhaseOutgoing".into(),
        seed: 37,
        replay_directory: directory.join("Replays"),
        exe_crc: 0,
        ini_crc: 0,
        save_stats: false,
        stats_directory: directory.join("Stats"),
    };
    // Rejected Replay admission is not a successful prepared start and must
    // not publish mode/difficulty/FPS or consume a pending map into the world.
    engine.replay_fast_forward = false;
    install_replay_factory(engine);
    engine
        .game_logic
        .prepare_native_command_mode(GameMode::Skirmish);
    engine.game_logic.prepare_native_command_difficulty(0);
    engine.game_logic.frame = 37;
    engine
        .game_logic
        .replay_pending
        .reset_recorder_owned(inputs.clone());
    engine.current_state = GameState::InGame;
    engine.pending_match_start = None;
    // A malformed typed lobby reaches the receiving preparation boundary
    // after the descriptor seed, difficulty and FPS have been applied.
    // Its rejected admission must restore the existing stream in place.
    engine.game_logic.set_logic_random_seed(0x3141_5926);
    let _ = engine.game_logic.logic_random.next_u32();
    let prior_base_seed = engine.game_logic.logic_base_seed;
    let prior_rng_words = engine.game_logic.logic_random.seed_words();
    let prior_map_fallback = Some("Maps/PreviousReceivingFallback".to_string());
    engine.application_map_fallback = prior_map_fallback.clone();
    let mut invalid_shell = GameMessage::new(GameMessageType::NewGame);
    for value in [4, 2, 71, 60] {
        invalid_shell.append_integer_argument(value);
    }
    invalid_shell.set_shell_map_launch(ClientShellMapLaunch {
        map: String::new(),
        seed: 0x1618_0339,
    });
    assert!(
        engine
            .prepare_receiving_source_new_game(&invalid_shell)
            .is_err()
    );
    assert_eq!(engine.game_logic.logic_base_seed, prior_base_seed);
    assert_eq!(engine.game_logic.logic_random.seed_words(), prior_rng_words);
    assert_eq!(engine.application_map_fallback, prior_map_fallback);
    assert_eq!(engine.game_logic.game_mode(), GameMode::Skirmish);
    assert!(engine.pending_match_start.is_none());

    let mut invalid_lobby = GameMessage::new(GameMessageType::NewGame);
    for value in [2, 1, 0, 60] {
        invalid_lobby.append_integer_argument(value);
    }
    let mut invalid_game_info =
        game_engine::common::system::skirmish_game_info::SkirmishGameInfo::default();
    invalid_game_info.seed = 0x2718_2818;
    invalid_lobby.set_skirmish_launch(ClientSkirmishLaunch {
        map: "Maps/RejectedTypedLobby".into(),
        game_info: invalid_game_info,
        local_slot: 0,
        old_factions_only: false,
        fog_of_war: true,
        game_mode_code: 2,
        max_fps: 60,
    });
    assert!(
        engine
            .prepare_receiving_source_new_game(&invalid_lobby)
            .is_err()
    );
    assert_eq!(engine.game_logic.logic_base_seed, prior_base_seed);
    assert_eq!(engine.game_logic.logic_random.seed_words(), prior_rng_words);
    assert_eq!(engine.application_map_fallback, prior_map_fallback);
    assert_eq!(engine.game_logic.game_mode(), GameMode::Skirmish);
    assert!(engine.pending_match_start.is_none());

    // An owned map updates the application fallback before this mapless
    // fixture rejects skirmish setup without receiving multiplayer settings.
    assert!(engine.game_logic.multiplayer_definitions.is_none());
    engine.current_state = GameState::Menu;
    let prior_map_fallback = Some("Maps/BeforeRejectedTransition".to_string());
    engine.application_map_fallback = prior_map_fallback.clone();
    let mut selected_map = GameMessage::new(GameMessageType::NewGame);
    for value in [2, 1, 0, 60] {
        selected_map.append_integer_argument(value);
    }
    selected_map.set_owned_new_game_map("Maps/RejectedTransition".into());
    assert!(!engine.prepare_receiving_source_new_game(&selected_map).unwrap());
    assert_eq!(engine.application_map_fallback, prior_map_fallback);
    assert_eq!(engine.game_logic.logic_base_seed, prior_base_seed);
    assert_eq!(engine.game_logic.logic_random.seed_words(), prior_rng_words);
    assert!(engine.pending_match_start.is_none());
    assert_eq!(engine.current_state, GameState::Menu);
    engine.current_state = GameState::InGame;

    let fps_before = {
        let mut global = game_engine::common::global_data::write();
        global.pending_file = "Maps/RejectedReplayInput".into();
        (
            global.writable.use_fps_limit,
            global.writable.frames_per_second_limit,
        )
    };
    let mut rejected = GameMessage::new(GameMessageType::NewGame);
    for value in [3, 2, 71, 60] {
        rejected.append_integer_argument(value);
    }
    engine
        .game_logic
        .replay_pending
        .receive_client_messages(vec![rejected]);
    direct_probe_frame(engine, 4. * (1. / 30.)).expect("complete rejected replay frame");
    assert_eq!(
        engine.game_logic.frame, 41,
        "rejected admission completes all four original budgeted steps without a prepared-start guard"
    );
    assert_eq!(engine.game_logic.game_mode(), GameMode::Skirmish);
    assert_eq!(
        engine
            .game_logic
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_global_difficulty(),
        gamelogic::player::GameDifficulty::Easy
    );
    assert!(engine.pending_match_start.is_none());
    assert!(engine.source_new_game_background.is_none());
    {
        let global = game_engine::common::global_data::read();
        assert_eq!(global.pending_file, "Maps/RejectedReplayInput");
        assert_eq!(
            (
                global.writable.use_fps_limit,
                global.writable.frames_per_second_limit
            ),
            fps_before
        );
    }
    install_replay_factory(engine);
    engine
        .game_logic
        .prepare_native_command_mode(GameMode::Skirmish);
    engine.game_logic.frame = 37;
    engine
        .game_logic
        .replay_pending
        .reset_recorder_owned(inputs.clone());
    engine.current_state = GameState::InGame;
    engine
        .game_logic
        .replay_pending
        .receive_client_messages(vec![GameMessage::new(GameMessageType::ClearGameData)]);
    direct_probe_frame(engine, 1. / 30.).expect("complete clear-game frame");
    let mut score_frame = None;
    engine.game_client.window_manager_owner().with_manager(|manager| {
        score_frame = manager.find_window_by_name("ScoreScreen.wnd:ParentScoreScreen")
            .and_then(|window| window.borrow().get_user_data::<game_client::gui::callbacks::score_screen::ScoreScreenAcademySnapshot>().map(|snapshot| snapshot.end_frame));
    });
    assert_eq!(
        score_frame,
        Some(37),
        "actual source ScoreScreen must freeze BEFORE outgoing object callbacks and frame increment"
    );
    assert_eq!(engine.game_logic.game_mode(), GameMode::None);
    assert_eq!(
        engine.game_logic.frame, 1,
        "source Clear-only tail advances cleared frame0 once"
    );
    assert!(engine.game_logic.host_object(ObjectId(1)).is_none());
    assert!(engine.last_presentation_frame.as_ref().is_none_or(|frame| frame.objects.is_empty()));

    // Shell.cpp448–472 legitimately admits [CLEAR, NEW_GAME(SHELL)]. It
    // must survive subsystem reset and park exact Shell map loading.
    install_replay_factory(engine);
    engine
        .game_logic
        .prepare_native_command_mode(GameMode::Skirmish);
    engine.game_logic.frame = 37;
    engine
        .game_logic
        .replay_pending
        .reset_recorder_owned(inputs.clone());
    engine.current_state = GameState::InGame;
    let shell_map = "Maps/SourcePhaseShell/SourcePhaseShell.map";
    game_engine::common::global_data::write().pending_file = shell_map.into();
    let mut newgame = GameMessage::new(GameMessageType::NewGame);
    for value in [4, 2, 71, 60] {
        newgame.append_integer_argument(value);
    }
    engine
        .game_logic
        .replay_pending
        .receive_client_messages(vec![
            GameMessage::new(GameMessageType::ClearGameData),
            newgame,
        ]);
    direct_probe_frame(engine, 1. / 30.).expect("complete shell-start frame");
    assert_eq!(
        engine.game_logic.frame, 0,
        "actual prepared NewGame suppresses source frame increment"
    );
    assert_eq!(engine.game_logic.game_mode(), GameMode::Shell);
    let pending = engine
        .pending_match_start
        .as_ref()
        .expect("source Shell load parked, not ignored");
    assert_eq!(pending.request.mode, GameMode::Shell);
    assert_eq!(
        pending.request.map, shell_map,
        "never replace source Shell map with Defcon6"
    );
    assert_eq!(pending.request.recorder_dispatch.unwrap().rank_points, 71);
    assert_eq!(pending.request.difficulty_code, Some(2));
    assert_eq!(
        engine
            .game_logic
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_global_difficulty(),
        gamelogic::player::GameDifficulty::Hard
    );
    assert!(
        engine.source_new_game_background.is_some(),
        "source prepare created its actual receiving BlankWindow"
    );
    assert!(
        engine
            .game_logic
            .replay_pending
            .take_lifecycle_messages_owned()
            .is_empty(),
        "native source batch was not split into a delayed lifecycle queue"
    );
    engine.pending_match_start = None;
    engine.destroy_source_new_game_background();
    // Actual receiving manager, deliberately invalid layout only at this
    // test boundary: no first window may be admitted as a source background.
    install_replay_factory(engine);
    engine
        .game_logic
        .prepare_native_command_mode(GameMode::Skirmish);
    engine.game_logic.prepare_native_command_difficulty(0);
    engine.game_logic.frame = 37;
    engine
        .game_logic
        .replay_pending
        .reset_recorder_owned(inputs);
    engine.current_state = GameState::InGame;
    let mut invalid = None;
    let mut invalid_weak = None;
    engine
        .game_client
        .window_manager_owner()
        .with_manager(|manager| {
            let layout = manager.create_layout("<source-empty-background-regression>".into());
            invalid_weak = Some(std::rc::Rc::downgrade(&layout));
            invalid = Some(layout);
        });
    engine.source_new_game_background = invalid;
    let fps_before = {
        let mut global = game_engine::common::global_data::write();
        global.pending_file = "Maps/BackgroundFailureMustNotConsume".into();
        (
            global.writable.use_fps_limit,
            global.writable.frames_per_second_limit,
        )
    };
    let mut unavailable = GameMessage::new(GameMessageType::NewGame);
    for value in [4, 2, 71, 60] {
        unavailable.append_integer_argument(value);
    }
    engine
        .game_logic
        .replay_pending
        .receive_client_messages(vec![unavailable]);
    direct_probe_frame(engine, 1. / 30.).expect("complete rejected-background frame");
    assert_eq!(engine.game_logic.frame, 38);
    assert_eq!(engine.game_logic.game_mode(), GameMode::Skirmish);
    assert_eq!(
        engine
            .game_logic
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_global_difficulty(),
        gamelogic::player::GameDifficulty::Easy
    );
    assert!(engine.pending_match_start.is_none());
    assert!(engine.source_new_game_background.is_none());
    assert!(
        invalid_weak.unwrap().upgrade().is_none(),
        "failed actual background removed from receiving manager"
    );
    let global = game_engine::common::global_data::read();
    assert_eq!(global.pending_file, "Maps/BackgroundFailureMustNotConsume");
    assert_eq!(
        (
            global.writable.use_fps_limit,
            global.writable.frames_per_second_limit
        ),
        fps_before
    );
    drop(global);
    std::fs::remove_dir_all(directory).expect("remove owned source command inputs");
}

/// Opt-in licensed-content gate. A requested gate never substitutes a map
/// or reports success without traversing the actual parked Loading path.
#[cfg(feature = "game_client")]
fn source_shell_load_engine_witness(
    engine: &mut CnCGameEngine,
    startup_catalogs: super::super::StartupLoadState,
) -> anyhow::Result<bool> {
    use crate::cnc_game_engine::GameState;
    use crate::game_logic::GameMode;
    use game_engine::common::message_stream::{GameMessage, GameMessageType};
    if std::env::var_os("GENERALS_SHELL_LOAD_GATE").is_none() {
        return Ok(false);
    }
    anyhow::ensure!(
        std::env::var("GENERALS_SHELL_LOAD_GATE").as_deref() == Ok("1"),
        "GENERALS_SHELL_LOAD_GATE must be exactly 1 when requested"
    );
    let requested = std::env::var_os("GENERALS_SHELL_LOAD_GATE_MAP")
        .ok_or_else(|| anyhow::anyhow!("Shell load gate requires GENERALS_SHELL_LOAD_GATE_MAP"))?;
    let requested = std::path::PathBuf::from(requested);
    anyhow::ensure!(
        requested.is_absolute(),
        "Shell gate map must be an absolute licensed .map path"
    );
    let path = requested.canonicalize()?;
    anyhow::ensure!(
        path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("map")),
        "Shell gate requires an existing licensed .map file"
    );
    let map = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Shell map path is not UTF-8"))?
        .to_owned();
    let decoded = crate::game_logic::script_loader::load_chunky_map(&map)
        .map_err(|error| anyhow::anyhow!("Cannot decode selected Shell gate map: {error:?}"))?
        .ok_or_else(|| anyhow::anyhow!("Selected Shell gate map cannot be resolved"))?;
    assert_eq!(
        decoded.source, path,
        "selected absolute map resolves without substitution"
    );
    let expected = crate::game_logic::script_loader::parse_heightmap_data_from_chunky(&decoded)
        .map_err(|error| anyhow::anyhow!("Cannot decode selected Shell gate terrain: {error:?}"))?
        .ok_or_else(|| anyhow::anyhow!("Selected Shell gate map has no HeightMapData"))?;
    anyhow::ensure!(
        expected.width > 0 && expected.height > 0 && !expected.data.is_empty(),
        "Selected Shell gate map has no actual heightmap"
    );
    // The safe launcher has installed private writable userdata before
    // constructing this Engine. Do not run this gate through the old wrapper.
    let private = std::env::var_os("GENERALS_REPLAY_PROBE_PRIVATE_ROOT").ok_or_else(|| {
        anyhow::anyhow!("Shell gate requires the private-userdata probe launcher")
    })?;
    let directory = std::path::PathBuf::from(private).join("shell-load-gate");
    std::fs::create_dir_all(&directory)?;
    let inputs = game_engine::common::recorder::RecorderOwnerInputs {
        map: "Maps/SourcePhaseOutgoing".into(),
        seed: 37,
        replay_directory: directory.join("Replays"),
        exe_crc: 0,
        ini_crc: 0,
        save_stats: false,
        stats_directory: directory.join("Stats"),
    };
    // The preceding queue fixtures have no native births. Move the real
    // prepared application owner only when this map gate needs it, rather
    // than mixing synthetic queue templates into its strict catalog.
    anyhow::ensure!(
        !engine.game_logic.requires_native_drawable_owner(),
        "Shell probe cannot discard a preceding native world"
    );
    // Consume the actual captured Object/Science producer, stopping
    // before native map admission or any actor constructors.
    match startup_catalogs {
        super::super::StartupLoadState::InProgress { receiver, .. } => {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
            let mut fps_before_startup = None;
            let prepared = loop {
                anyhow::ensure!(
                    std::time::Instant::now() < deadline,
                    "probe startup catalogs exceeded admission deadline"
                );
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                let message = receiver
                    .recv_timeout(remaining)
                    .map_err(|error| anyhow::anyhow!("probe startup catalogs: {error}"))?;
                match message {
                    super::super::StartupLoadMessage::Progress { .. } => {}
                    super::super::StartupLoadMessage::ApplyFpsLimit(request) => {
                        if fps_before_startup.is_none() {
                            fps_before_startup = Some(engine.fps_limiter);
                        }
                        let resolved = engine.fps_limiter.apply_new_game(request.requested);
                        let _ = request.reply.send(resolved);
                    }
                    super::super::StartupLoadMessage::Prepared(result) => match result {
                        Ok(prepared) => break prepared,
                        Err(error) => {
                            if let Some(before) = fps_before_startup.take() {
                                engine.fps_limiter = before;
                                engine.fps_limiter_last_tick = None;
                            }
                            return Err(anyhow::Error::msg(error));
                        }
                    },
                }
            };
            anyhow::ensure!(
                prepared.game_logic.host_objects().registered_is_empty(),
                "startup catalog handoff unexpectedly admitted host Objects"
            );
            anyhow::ensure!(
                !prepared.game_logic.requires_native_drawable_owner(),
                "startup catalog handoff unexpectedly admitted native ownership"
            );
            anyhow::ensure!(
                prepared.game_logic.receiving_module_content.is_some()
                    && prepared.game_logic.receiving_object_definitions.is_some()
                    && prepared.game_logic.receiving_template_baseline.is_some(),
                "requested Shell probe requires paired startup Object content"
            );
            engine.game_logic = prepared.game_logic;
        }
        _ => anyhow::bail!("requested Shell probe requires startup catalog producer"),
    }
    engine
        .game_logic
        .prepare_native_command_mode(GameMode::Skirmish);
    engine.game_logic.prepare_native_command_difficulty(0);
    engine.game_logic.frame = 37;
    engine
        .game_logic
        .session_view
        .camera
        .native_camera
        .set_height(12345., 0., 50000.);
    assert_eq!(
        engine
            .game_logic
            .session_view
            .camera
            .native_camera
            .desired(),
        12345.
    );
    engine
        .game_logic
        .replay_pending
        .reset_recorder_owned(inputs);
    // Begin at the existing probe's actual outgoing InGame state; from
    // this notification onward only production admission changes state.
    assert_eq!(engine.current_state, GameState::InGame);
    game_engine::common::global_data::write().pending_file = map.clone().into();
    let mut newgame = GameMessage::new(GameMessageType::NewGame);
    for value in [4, 2, 71, 60] {
        newgame.append_integer_argument(value);
    }
    engine
        .game_logic
        .replay_pending
        .receive_client_messages(vec![
            GameMessage::new(GameMessageType::ClearGameData),
            newgame,
        ]);
    longevity_frame(engine)?;
    assert_eq!(engine.current_state, GameState::Loading);
    assert_eq!(engine.game_logic.game_mode(), GameMode::Shell);
    assert_eq!(
        engine.game_logic.frame, 0,
        "prepared start suppresses outgoing frame increment"
    );
    assert_ne!(
        engine
            .game_logic
            .session_view
            .camera
            .native_camera
            .desired(),
        12345.,
        "outgoing HAG goal must be retired by receiving reset"
    );
    let pending = engine
        .pending_match_start
        .as_ref()
        .expect("actual parked Shell descriptor");
    assert_eq!(
        pending.request.map, map,
        "no Defcon/default map substitution"
    );
    assert_eq!(pending.request.difficulty_code, Some(2));
    assert!(engine.source_new_game_background.is_some());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while engine.current_state == GameState::Loading {
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "actual Shell Loading did not complete for {map}"
        );
        engine.host_tick_loading_client_residuals()?;
        if engine.current_state == GameState::Loading {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    assert_eq!(engine.current_state, GameState::Menu);
    assert!(engine.pending_match_start.is_none());
    assert!(engine.source_new_game_background.is_none());
    assert_eq!(engine.game_logic.game_mode(), GameMode::Shell);
    assert!(
        engine.game_logic.isInGame(),
        "real selected map must be playable, not merely named"
    );
    assert_eq!(engine.game_logic.get_current_map_name(), map);
    assert_eq!(
        engine
            .game_logic
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_global_difficulty(),
        gamelogic::player::GameDifficulty::Hard,
        "actual map script replacement must retain prepared source difficulty"
    );
    assert!(
        engine.game_logic.scripts_loaded,
        "real map script foundation completed"
    );
    {
        let terrain = engine
            .game_logic
            .world_services
            .terrain()
            .read()
            .map_err(|_| anyhow::anyhow!("receiving Shell terrain poisoned"))?;
        assert!(terrain.has_height_map());
        let extent = terrain.get_extent_including_border();
        assert_eq!(extent.hi.x - extent.lo.x, expected.width as f32 * 10.);
        assert_eq!(extent.hi.y - extent.lo.y, expected.height as f32 * 10.);
    }
    let goal = engine
        .game_logic
        .session_view
        .camera
        .native_camera
        .desired();
    assert!(
        goal.is_finite() && goal != 12345.,
        "loaded camera belongs to receiving map"
    );
    assert!(engine.game_logic.session_view.target.is_finite());
    assert_eq!(
        engine.game_logic.frame, 0,
        "Loading does not consume a gameplay frame"
    );
    longevity_frame(engine)?;
    assert_eq!(
        engine.game_logic.frame, 1,
        "prepared-start guard is released after real load"
    );
    assert!(engine.pending_match_start.is_none());
    println!(
        "source_shell_load_engine_witness passed: map={map}, terrain={}x{}, difficulty=Hard, frame=1",
        expected.width, expected.height
    );
    Ok(true)
}

#[cfg(feature = "game_client")]
fn longevity_frame(engine: &mut CnCGameEngine) -> anyhow::Result<()> {
    // update() completes the Client continuation before optional GPU presentation.
    engine.native_display.require_shell_slot().map_err(anyhow::Error::msg)?;
    engine.update(1. / 30.).map_err(anyhow::Error::msg)?;
    engine.native_display.require_shell_slot().map_err(anyhow::Error::msg)?;
    if engine.should_present_w3d_frame() {
        engine.render()?;
    }
    Ok(())
}

/// Exercise the real Shell -> ordered clear/new-game -> retail map continuation.
/// This gate uses normal lobby slots and authored starting objects, not a
/// fabricated army or an altered victory timer.
#[cfg(feature = "game_client")]
fn shell_to_skirmish_longevity_witness(engine: &mut CnCGameEngine) -> anyhow::Result<()> {
    use crate::cnc_game_engine::GameState;
    use crate::game_logic::GameMode;
    use game_client::gui::skirmish_setup::{SkirmishSetup, snapshot_skirmish_lobby};
    use game_client::SlotState;
    use game_engine::common::message_stream::{ClientSkirmishLaunch, GameMessage, GameMessageType};

    let Some(map) = std::env::var_os("GENERALS_SKIRMISH_LONGEVITY_GATE_MAP") else {
        return Ok(());
    };
    let map = std::path::PathBuf::from(map).canonicalize()?;
    let map = map.to_str().ok_or_else(|| anyhow::anyhow!("map path is not UTF-8"))?.to_owned();
    let definitions = engine.game_logic.player_template_definitions.as_ref()
        .ok_or_else(|| anyhow::anyhow!("retail PlayerTemplate definitions unavailable"))?;
    let america = definitions.find_template_index("FactionAmerica")
        .ok_or_else(|| anyhow::anyhow!("FactionAmerica unavailable"))? as i32;
    let gla = definitions.find_template_index("FactionGLA")
        .ok_or_else(|| anyhow::anyhow!("FactionGLA unavailable"))? as i32;

    // Reproduce the actual stack present under the skirmish options screen.
    engine.with_receiving_menu_shell(|shell, effects| {
        shell.reset_receiving(effects).map_err(|error| error.to_string())?;
        shell.push("Menus/MainMenu.wnd", true, effects).map_err(|error| error.to_string())?;
        shell.push("Menus/SkirmishGameOptionsMenu.wnd", true, effects).map_err(|error| error.to_string())?;
        Ok::<_, String>(())
    }).map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("missing Shell"))?
        .map_err(anyhow::Error::msg)?;

    let mut setup = SkirmishSetup::default();
    setup.set_selected_map(map.clone());
    let info = setup.game_info_mut().game_info_mut();
    info.enter_game();
    info.set_map(map.clone());
    info.set_seed(73);
    for (index, state, name, template) in [
        (0, SlotState::Player, "Player", america),
        (1, SlotState::MedAI, "Medium Army", gla),
    ] {
        let slot = info.get_slot_mut(index).expect("retail lobby slot");
        slot.set_state(state, name.into(), if index == 0 { 1 } else { 0 });
        slot.set_player_template(template);
        slot.set_color(index as i32);
        slot.set_start_pos(index as i32);
        slot.set_team_number(index as i32);
        slot.save_off_original_info();
    }
    info.set_local_ip(1);
    info.start_game(0);
    let mut new_game = GameMessage::new(GameMessageType::NewGame);
    for value in [2, 1, 0, 30] { new_game.append_integer_argument(value); }
    new_game.set_skirmish_launch(ClientSkirmishLaunch {
        map: map.clone(),
        game_info: snapshot_skirmish_lobby(&setup),
        local_slot: 0,
        old_factions_only: false,
        fog_of_war: true,
        game_mode_code: 2,
        max_fps: 30,
    });
    let stream = engine.game_client.message_stream_mut();
    stream.append_message(GameMessageType::ClearGameData);
    stream.append_message_value(new_game);
    longevity_frame(engine)?;
    assert_eq!(engine.current_state, GameState::Loading);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while engine.current_state == GameState::Loading {
        anyhow::ensure!(std::time::Instant::now() < deadline, "skirmish loading timed out");
        longevity_frame(engine)?;
    }
    assert_eq!(engine.current_state, GameState::InGame);
    assert_eq!(engine.game_logic.get_current_map_name(), map);
    for player in [0, 1] {
        assert!(engine.game_logic.host_objects().registered_values().any(|object|
            object.owner_player_id == Some(player)
                && object.is_alive()
                && object.is_kind_of(KindOf::Structure)
                && object.is_kind_of(KindOf::MpCountForVictory)
        ), "player {player} must receive an authored victory-counting starting building");
    }
    for frame in 1..=8 {
        longevity_frame(engine)?;
        assert_eq!(engine.current_state, GameState::InGame, "match left gameplay at frame {frame}");
        assert_eq!(engine.game_logic.game_mode(), GameMode::Skirmish);
        assert!(engine.pending_match_start.is_none(), "shell game queued at frame {frame}");
        assert!(!engine.game_logic.current_victory_observation().match_over, "two live armies must not end the match");
    }
    assert!(engine.game_logic.frame > 1, "real match did not advance beyond its first logic frame");
    println!("shell_to_skirmish_longevity_witness passed: map={map}, frame={}", engine.game_logic.frame);
    Ok(())
}

/// Exercise the actual native archive and the first receiving CPU view Draw.
#[cfg(feature = "game_client")]
fn native_drawable_loco_archive_witness(engine: &mut CnCGameEngine) -> anyhow::Result<()> {
    use game_client::core::{DrawableId, GameClient};
    use game_client::drawable::{BasicDrawable, LocoInfo, WheelInfo};
    use game_client::physics_visual::{
        LiveClientRng, PhysicsVisualAppearance, calc_physics_visual_xform,
        glam_yup_physics_visual_local,
    };

    const TEMPLATE: &str = "AmericaVehicleHumvee";
    const SLOT: &str = "native-drawable-loco-continuation";
    let loco = |client: &GameClient, object: ObjectId| {
        let drawable = client.get_drawable_for_object(object.0)?;
        client
            .find_drawable_by_id(drawable)?
            .as_any()
            .downcast_ref::<BasicDrawable>()?
            .get_loco_info()
            .cloned()
    };
    let set_loco = |client: &mut GameClient, object: ObjectId, state: LocoInfo| {
        let drawable = client
            .get_drawable_for_object(object.0)
            .expect("object Drawable");
        *client
            .find_drawable_by_id_mut(drawable)
            .expect("receiving Drawable")
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .expect("BasicDrawable")
            .get_loco_info_mut() = state;
    };
    let draw = |engine: &mut CnCGameEngine, id: ObjectId| -> anyhow::Result<glam::Mat4> {
        engine
            .host_finalize_presentation_after_logic()
            .map_err(anyhow::Error::msg)?;
        engine
            .game_client
            .set_frame(engine.game_client.get_frame() + 1);
        engine
            .complete_native_display_step()
            .map_err(anyhow::Error::msg)?;
        let key = engine
            .game_client
            .presentation_direct_drawable_state(engine.host_direct_visual_world_epoch, id.0)
            .expect("native binding for the admitted actor")
            .binding_key;
        engine
            .game_client
            .borrow_native_drawable_owner()
            .native_draw_mesh_pose(key)
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .1
            .ok_or_else(|| {
                anyhow::anyhow!("the actual native CPU Draw did not complete Humvee physics")
            })
    };

    let mut spawn = engine.game_logic.session_view.target;
    spawn.y = engine
        .game_logic
        .terrain_height_at(spawn)
        .unwrap_or_default();
    let id = engine
        .host_create_object(TEMPLATE, Team::USA, spawn)
        .expect("actual stock Humvee with a native Drawable");
    // Let the local unit stamp its visibility before sampling the view phase.
    longevity_frame(engine)?;
    let before_cpu = loco(&engine.game_client, id);
    let source_pose = draw(engine, id)?;
    let source_loco = loco(&engine.game_client, id);
    let source_epoch = engine.host_direct_visual_world_epoch;

    let mut peer_world = GameLogic::new();
    peer_world.add_object(crate::game_logic::Object::new(
        ThingTemplate::new(TEMPLATE),
        id,
        Team::USA,
    ));
    let mut peer_client = GameClient::new()?;
    let peer_drawable =
        peer_client.register_drawable(Box::new(BasicDrawable::new(DrawableId(0))))?;
    peer_client.bind_drawable_to_object(peer_drawable, id.0)?;
    let peer_loco = LocoInfo {
        pitch: -0.667,
        yaw: -0.91,
        ..Default::default()
    };
    set_loco(&mut peer_client, id, peer_loco.clone());
    let peer_random = peer_world.session_view.client_random.seed_words();

    // Fill fields a single ordinary pose does not vary, and keep the native
    // archive layout test independent from any particular terrain slope.
    let saved = LocoInfo {
        pitch: 0.11,
        pitch_rate: 0.12,
        roll: -0.13,
        roll_rate: -0.14,
        yaw: 0.15,
        acceleration_pitch: 0.16,
        acceleration_pitch_rate: 0.17,
        acceleration_roll: -0.18,
        acceleration_roll_rate: -0.19,
        overlap_z_velocity: -0.20,
        overlap_z: 2.1,
        wobble: -1.0,
        yaw_modulator: 3.7,
        pitch_modulator: 4.6,
        wheel_info: WheelInfo {
            front_left_height_offset: 0.23,
            front_right_height_offset: 0.24,
            rear_left_height_offset: 0.25,
            rear_right_height_offset: 0.26,
            wheel_angle: 0.27,
            frames_airborne_counter: 28,
            frames_airborne: 29,
        },
    };
    set_loco(&mut engine.game_client, id, saved.clone());
    engine
        .host_save_game_from_ui(SLOT, "Native Drawable locomotor continuation")
        .map_err(anyhow::Error::msg)?;
    let decoded = engine.save_file_manager.load_game_snapshot(SLOT)?;
    let archive = decoded
        .game_client_archive
        .as_ref()
        .expect("full CHUNK_GameClient saved");
    crate::save_load::snapshot::validate_game_client_archive(archive)?;
    // The outgoing same numeric ObjectId now differs from the saved actor.
    set_loco(&mut engine.game_client, id, LocoInfo::default());
    engine
        .host_load_game_authority(SLOT)
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    assert_ne!(engine.host_direct_visual_world_epoch, source_epoch);

    let mut restored = saved;
    // These two oscillators are runtime-only in the original Xfer.
    restored.yaw_modulator = 0.0;
    restored.pitch_modulator = 0.0;
    assert_eq!(loco(&engine.game_client, id), Some(restored.clone()));
    engine
        .host_finalize_presentation_after_logic()
        .map_err(anyhow::Error::msg)?;
    let facts = engine
        .last_presentation_frame
        .as_ref()
        .expect("receiving frame")
        .physics_visual_facts(id)
        .expect("receiving Humvee physics facts");
    assert_eq!(facts.appearance, PhysicsVisualAppearance::WheelsFour);
    assert!(facts.show_client_physics && !facts.object_disabled_held);
    let mut expected_loco = restored.clone();
    let mut expected_random = engine.game_logic.session_view.client_random.clone();
    let expected_xform = calc_physics_visual_xform(
        facts.appearance,
        &mut expected_loco,
        &facts.params,
        &facts.body,
        &mut LiveClientRng(&mut expected_random),
    )
    .expect("one physics step from the archived state");
    let mut cold_loco = LocoInfo::default();
    let cold_xform = calc_physics_visual_xform(
        facts.appearance,
        &mut cold_loco,
        &facts.params,
        &facts.body,
        &mut LiveClientRng(&mut engine.game_logic.session_view.client_random.clone()),
    )
    .expect("zero history is a distinct control");
    let expected_pose = glam_yup_physics_visual_local(expected_xform);
    assert_ne!(expected_pose, glam_yup_physics_visual_local(cold_xform));
    assert_ne!(expected_pose, source_pose);
    assert_eq!(
        draw(engine, id)?,
        expected_pose,
        "first receiving CPU Draw discarded archive history"
    );
    assert_eq!(loco(&engine.game_client, id), Some(expected_loco.clone()));
    engine
        .complete_native_display_step()
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        loco(&engine.game_client, id),
        Some(expected_loco),
        "same Client frame advanced twice"
    );
    assert!(
        before_cpu.is_some() || source_loco.is_some(),
        "source CPU Draw never wrote its canonical LocoInfo"
    );
    assert_eq!(loco(&peer_client, id), Some(peer_loco));
    assert_eq!(
        peer_world.session_view.client_random.seed_words(),
        peer_random
    );
    println!(
        "native Drawable archive restored all 19 C++ values and continued the first CPU Draw for Object {}",
        id.0
    );
    Ok(())
}

fn actual_team_failure_stops_native_shell_and_fast_forward(engine: &mut CnCGameEngine) {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    use generals_sim::scripting::program::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    let peer = crate::game_logic::GameLogic::new();
    let peer_frame = peer.get_frame();
    assert!(engine.game_logic.requires_native_drawable_owner());
    assert!(
        engine.game_logic.get_player(14).is_none(),
        "probe needs unoccupied valid receiving Player slot14"
    );
    let mut player = crate::game_logic::Player::new(14, Team::USA, "DriverFailureOwner", false);
    player.map_side.map_player_name = "DriverFailureOwner".into();
    engine.game_logic.add_player(player);
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "DriverFailureCreate");
    engine
        .game_logic
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "DriverFailureTeam".into(),
            "DriverFailureOwner".into(),
            false,
            Some(&dict),
            Some(14),
        )
        .unwrap();
    fn count(name: &str) -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(ParameterType::Counter, name.into()))
            .unwrap();
        action
    }
    fn create() -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::CreateObject);
        for parameter in [
            Parameter::with_string(ParameterType::ObjectType, "AmericaTankCrusader".into()),
            Parameter::with_string(ParameterType::Team, "DriverFailureTeam".into()),
            Parameter::with_coord(
                ParameterType::Coord3D,
                glam::Vec3::new(1., 2., 3.),
            ),
            Parameter::with_real(ParameterType::Angle, 0.),
        ] {
            action.add_parameter(parameter).unwrap();
        }
        action
    }
    // The real nested factory creates a native-bound child, then this
    // malformed authored action fails at Main's existing typed boundary.
    let mut invalid = ScriptAction::new(ScriptActionType::TeamSetAttitude);
    invalid
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "DriverFailureTeam".into(),
        ))
        .unwrap();
    invalid
        .add_parameter(Parameter::with_int(ParameterType::Int, 999))
        .unwrap();
    let mut child = create();
    child.next_action = Some(Box::new(invalid));
    let mut visit = count("DriverFailureVisits");
    visit.next_action = Some(Box::new(child));
    let mut on_create = Script::new();
    on_create.script_name = "DriverFailureCreate".into();
    on_create.is_subroutine = true;
    on_create.action = Some(Box::new(visit));
    let mut first = count("DriverFailureTrigger");
    first.next_action = Some(Box::new(create()));
    let mut condition = Condition::new(ConditionType::Counter);
    for parameter in [
        Parameter::with_string(ParameterType::Counter, "DriverFailureTrigger".into()),
        Parameter::with_int(ParameterType::Comparison, 2),
        Parameter::with_int(ParameterType::Int, 0),
    ] {
        condition.add_parameter(parameter).unwrap();
    }
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut scheduled = Script::new();
    scheduled.script_name = "ScheduledFirstDriverFailure".into();
    scheduled.frame_to_evaluate_at = engine.game_logic.get_frame();
    scheduled.condition = Some(Box::new(branch));
    scheduled.action = Some(Box::new(first));
    let mut list = ScriptList::new();
    list.append_script(Box::new(on_create));
    list.append_script(Box::new(scheduled));
    let mut interpreter = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    for counter in ["DriverFailureVisits", "DriverFailureTrigger"] {
        interpreter.set_counter(counter, 0).unwrap();
    }
    interpreter
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine.game_logic.install_owned_script_engine(interpreter);
    let frame = engine.game_logic.get_frame();
    let old_ids = engine
        .game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let old_presentation = engine.last_presentation_frame.clone();
    assert!(
        engine.game_logic.team_creation_error().is_none(),
        "failure must arise inside first actual offered step"
    );
    engine.replay_fast_forward = true;
    engine.game_paused = false;
    let error = engine
        .host_run_ingame_logic_presentation_frame(4. / 30.)
        .unwrap_err();
    assert!(
        error.contains("attitude") && error.contains("999"),
        "{error}"
    );
    assert_eq!(
        engine.game_logic.get_frame(),
        frame,
        "failed early first step cannot commit or run steps2-4"
    );
    match (&old_presentation, &engine.last_presentation_frame) {
        (Some(old), Some(now)) => assert!(std::rc::Rc::ptr_eq(old, now)),
        (None, None) => {}
        _ => panic!("failed step published a new presentation"),
    }
    let team = engine
        .game_logic
        .team_factory
        .lock()
        .unwrap()
        .get_all_teams()
        .into_iter()
        .find(|team| team.read().unwrap().get_name().as_str() == "DriverFailureTeam")
        .unwrap();
    let identity = team.read().unwrap().get_id();
    assert!(team.read().unwrap().creation_failure().is_some());
    let births = engine
        .game_logic
        .host_objects()
        .registered()
        .filter(|(id, _)| !old_ids.contains(id))
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    assert_eq!(
        births.len(),
        1,
        "nested actual factory birth remains owned; outer failed constructor did not run"
    );
    let binding = engine
        .game_client
        .presentation_direct_drawable_state(engine.host_direct_visual_world_epoch, births[0].0)
        .unwrap()
        .binding_key;
    let count_before = engine.game_logic.host_objects().registered_len();
    assert!(engine.host_update_shell_with_budget(1. / 30., 4).is_err());
    assert_eq!(engine.game_logic.get_frame(), frame);
    assert_eq!(
        engine.game_logic.host_objects().registered_len(),
        count_before
    );
    assert_eq!(
        engine
            .game_client
            .presentation_direct_drawable_state(engine.host_direct_visual_world_epoch, births[0].0)
            .unwrap()
            .binding_key,
        binding
    );
    let same = engine
        .game_logic
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(identity)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&same, &team));
    let interpreter = engine.game_logic.owned_script_engine.as_ref().unwrap();
    assert_eq!(
        interpreter
            .get_counter("DriverFailureVisits")
            .unwrap()
            .value,
        1
    );
    assert_eq!(
        interpreter
            .get_counter("DriverFailureTrigger")
            .unwrap()
            .value,
        1
    );
    assert!(
        crate::save_load::SnapshotBuilder::new()
            .create_world_snapshot_with_client(&engine.game_logic, &engine.game_client)
            .is_err()
    );
    assert_eq!(peer.get_frame(), peer_frame);
    assert!(peer.team_creation_error().is_none());
    assert!(peer.host_objects().registered_is_empty());
    println!(
        "actual_team_failure_stops_native_shell_and_fast_forward passed: frame={frame}, TeamID={identity:?}"
    );
}

#[cfg(feature = "game_client")]
fn visual_physics_display_witness(engine: &mut CnCGameEngine) -> anyhow::Result<()> {
    use crate::cnc_game_engine::GameState;
    use crate::game_logic::{GameMode, locomotor_bootstrap::apply_host_locomotor_binding};
    use game_client::drawable::drawable::BasicDrawable;
    use game_client::physics_visual::PhysicsVisualAppearance;
    use game_engine::common::ini::ini_locomotor::{LocomotorStore, load_locomotors_from_str};

    const OBJECT: &str = "TivoVisualPhysicsCar";
    const LOCOMOTOR: &str = "TivoVisualPhysicsWheels";
    let admit = |world: &mut GameLogic| -> anyhow::Result<()> {
        let mut locomotors = LocomotorStore::new_owned();
        load_locomotors_from_str(
            &mut locomotors,
            "Locomotor TivoVisualPhysicsWheels\nSurfaces = GROUND\nSpeed = 60\nAppearance = FOUR_WHEELS\nBounceAmount = 0.4\nEnd\n",
        )?;
        world
            .admit_locomotor_definitions(locomotors)
            .map_err(anyhow::Error::msg)?;
        world.admit_object_ini_sources([(
            "TivoVisualPhysics.ini".into(),
            "Object TivoVisualPhysicsCar\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = None\nEnd\nEnd\nEnd\n".into(),
        )])?;
        world
            .start_new_game(GameMode::Replay)
            .map_err(anyhow::Error::msg)?;
        world.session_view.client_random.seed_random(17);
        Ok(())
    };
    let move_car = |world: &mut GameLogic, id: ObjectId| {
        let binding = world.resolve_receiving_locomotor(LOCOMOTOR).unwrap();
        let car = world.host_object_mut(id).unwrap();
        apply_host_locomotor_binding(car, &binding);
        car.movement.velocity = Vec3::X * car.movement.max_speed;
        car.set_ground_height_residual(0., false);
        car.queue_second_material_pass(1.);
    };

    engine.abandon_startup_load_worker();
    admit(&mut engine.game_logic)?;
    engine.game_logic.require_native_drawable_owner();
    engine.current_state = GameState::InGame;
    engine.replay_fast_forward = true;
    engine.game_logic.frame = 2;
    engine.game_client.set_frame(2);

    let visible = engine
        .host_create_object(OBJECT, Team::USA, Vec3::new(1., 0., 1.))
        .expect("native visible actor");
    let hidden = engine
        .host_create_object(OBJECT, Team::USA, Vec3::new(3., 0., 1.))
        .expect("native hidden actor");
    let outside = engine
        .host_create_object(OBJECT, Team::USA, Vec3::new(10_000., 0., 10_000.))
        .expect("native outside actor");
    for id in [visible, hidden, outside] {
        move_car(&mut engine.game_logic, id);
    }

    let mut peer = GameLogic::new();
    admit(&mut peer)?;
    let peer_car = peer
        .create_object(OBJECT, Team::USA, Vec3::new(1., 0., 1.))
        .expect("independent peer actor");
    assert_eq!(peer_car, visible);
    move_car(&mut peer, peer_car);
    let peer_random = peer.session_view.client_random.seed_words();

    engine
        .host_finalize_presentation_after_logic()
        .map_err(anyhow::Error::msg)?;
    let frame = engine
        .last_presentation_frame
        .as_ref()
        .expect("published replay observation");
    assert!(frame.in_replay_game);
    for id in [visible, hidden, outside] {
        let facts = frame
            .physics_visual_facts(id)
            .expect("receiving visual physics inputs");
        assert_eq!(facts.appearance, PhysicsVisualAppearance::WheelsFour);
        assert!(facts.show_client_physics && !facts.body.significantly_above_terrain);
        assert!(facts.params.bounce_kick > 0. && facts.body.cur_locomotor_speed > 0.);
        assert!(facts.body.velocity_magnitude > facts.body.cur_locomotor_speed / 10.);
    }

    let epoch = engine.host_direct_visual_world_epoch;
    let hidden_drawable = engine
        .game_client
        .presentation_direct_drawable_state(epoch, hidden.0)
        .expect("hidden actor has native binding")
        .binding_key
        .drawable_id;
    engine
        .game_client
        .find_drawable_by_id_mut(hidden_drawable)
        .expect("hidden receiving Drawable")
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .expect("typed receiving Drawable")
        .set_drawable_hidden(true);

    let before = engine.game_logic.session_view.client_random.seed_words();
    let mut expected = engine.game_logic.session_view.client_random.clone();
    expected.next_int(0, 3);
    assert_ne!(before, expected.seed_words());
    let opacity = |engine: &CnCGameEngine, id: ObjectId| {
        engine
            .game_client
            .capture_presentation_material_opacities()
            .into_iter()
            .find(|(key, _)| key.host_epoch == epoch && key.object_id == id.0)
            .expect("bound Drawable material state")
            .1
    };

    engine
        .complete_native_display_step()
        .map_err(anyhow::Error::msg)?;
    assert!(
        !engine.should_present_w3d_frame(),
        "TiVO at replay frame 2 must deny the GPU"
    );
    assert_eq!(
        opacity(engine, visible),
        0.8,
        "visible actor entered CPU Draw"
    );
    assert_eq!(
        opacity(engine, hidden),
        0.8,
        "in-region hidden actor faded before the visibility return"
    );
    assert_eq!(
        opacity(engine, outside),
        1.,
        "out-of-region actor did not enter CPU Draw"
    );
    let draw_pose = |engine: &mut CnCGameEngine, id: ObjectId| {
        let key = engine.game_client.presentation_direct_drawable_state(epoch, id.0)
            .expect("native binding for completed Draw").binding_key;
        engine.game_client.borrow_native_drawable_owner().native_draw_mesh_pose(key)
            .expect("completed CPU Draw pose").1
    };
    let visible_pose = draw_pose(engine, visible).expect("wheel physics completed during CPU Draw");
    assert!(visible_pose.is_finite());
    assert_ne!(visible_pose, glam::Mat4::IDENTITY);
    assert_eq!(draw_pose(engine, hidden), None);
    assert_eq!(draw_pose(engine, outside), None);
    let after = engine.game_logic.session_view.client_random.seed_words();
    engine
        .complete_native_display_step()
        .map_err(anyhow::Error::msg)?;
    assert!(!engine.should_present_w3d_frame());
    assert_eq!(
        opacity(engine, visible),
        0.8,
        "repeat Client frame did not offer another CPU Draw"
    );
    assert_eq!(
        engine.game_logic.session_view.client_random.seed_words(),
        after,
        "repeat Client frame did not consume RNG"
    );
    assert_eq!(
        peer.session_view.client_random.seed_words(),
        peer_random,
        "same-ID peer remained untouched"
    );
    assert_eq!(
        after,
        expected.seed_words(),
        "admitted native CPU Draw must consume one wheel-bounce RNG draw even when TiVO denies the GPU"
    );
    assert_eq!(draw_pose(engine, visible), Some(visible_pose));

    // Switch back to ordinary display without advancing Client time. The
    // existing render route can submit/retry while the completed pose stays put.
    engine.replay_fast_forward = false;
    engine.complete_native_display_step().map_err(anyhow::Error::msg)?;
    assert!(engine.should_present_w3d_frame());
    for _ in 0..2 {
        engine.render()?;
        assert_eq!(draw_pose(engine, visible), Some(visible_pose));
        assert_eq!(engine.game_logic.session_view.client_random.seed_words(), after);
    }
    println!("native CPU Draw advanced exactly one wheel-bounce RNG draw on a TiVO GPU-skip frame");
    Ok(())
}

pub(super) fn run_replay_fast_forward_engine_probe() -> anyhow::Result<()> {
    if std::env::var_os("GENERALS_SKIRMISH_LONGEVITY_GATE_MAP").is_some() {
        eprintln!("Running skirmish longevity only: replay fast-forward, command-phase and team-failure regressions are NOT RUN");
        anyhow::ensure!(
            cfg!(feature = "game_client")
                && std::env::var("GENERALS_SHELL_LOAD_GATE").as_deref() == Ok("1"),
            "skirmish longevity gate requires game_client and GENERALS_SHELL_LOAD_GATE=1"
        );
    }
    let (_event_loop, _application_services, mut engine) = replay_fixture_engine()?;
    #[cfg(feature = "game_client")]
    if std::env::var_os("GENERALS_TIVO_VISUAL_PHYSICS_GATE").is_some() {
        return visual_physics_display_witness(&mut engine);
    }
    #[cfg(feature = "game_client")]
    if std::env::var_os("GENERALS_SKIRMISH_LONGEVITY_GATE_MAP").is_some() {
        // Keep the actual startup worker and admit its ordered retail content
        // through the same native map/Client boundary as an application boot.
        // The queue/replay fixtures below replace GameLogic, so they cannot be
        // used as the owner for this match-start regression.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        while matches!(engine.startup_load_state, super::super::StartupLoadState::InProgress { .. }) {
            anyhow::ensure!(std::time::Instant::now() < deadline, "retail startup timed out");
            engine.update_startup_loading()?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        anyhow::ensure!(
            engine.current_state == crate::cnc_game_engine::GameState::Menu
                && engine.game_logic.game_mode() == crate::game_logic::GameMode::Shell,
            "retail startup did not complete a Shell game"
        );
        let requested = std::env::var_os("GENERALS_SHELL_LOAD_GATE_MAP")
            .ok_or_else(|| anyhow::anyhow!("Shell load gate requires GENERALS_SHELL_LOAD_GATE_MAP"))?;
        let expected = std::path::PathBuf::from(requested).canonicalize()?;
        let actual = crate::game_logic::script_loader::load_chunky_map(
            engine.game_logic.get_current_map_name(),
        )
        .map_err(|error| anyhow::anyhow!("Cannot resolve startup Shell map: {error:?}"))?
        .ok_or_else(|| anyhow::anyhow!("Startup Shell map cannot be resolved"))?
        .source.canonicalize()?;
        anyhow::ensure!(actual == expected, "startup Shell map differs from requested gate map");
        shell_to_skirmish_longevity_witness(&mut engine)?;
        if std::env::var_os("GENERALS_NATIVE_DRAWABLE_LOCO_SAVE_GATE").is_some() {
            native_drawable_loco_archive_witness(&mut engine)?;
        }
        return Ok(());
    }
    // Keep the real startup producer owned outside synthetic command fixtures,
    // whose reset paths legitimately cancel their own pending startup work.
    let startup_catalogs = std::mem::replace(
        &mut engine.startup_load_state,
        super::super::StartupLoadState::Complete,
    );
    #[cfg(feature = "game_client")]
    {
        engine.game_logic.admit_object_ini_sources([(
            "ReplayCommandOwner.ini".into(),
            "Object ReplayCommandOwnerFixture\nEnd\n".into(),
        )])?;
        engine
            .probe_skirmish_local_command_owner()
            .map_err(anyhow::Error::msg)?;
    }
    install_replay_factory(&mut engine);
    // Actual Engine startup/hotkey/native continuation, executed only by
    // the existing process-main-thread probe. This is not a unit mock.
    let old_replay = engine.host_match_in_replay;
    engine.host_match_in_replay = Some(true);
    let old_global = game_engine::common::global_data::read().tivo_fast_mode;
    game_engine::common::global_data::write().tivo_fast_mode = false;
    engine.replay_fast_forward = false;
    engine.toggle_replay_fast_forward_hotkey();
    assert!(engine.replay_fast_forward);
    assert!(
        !game_engine::common::global_data::read().tivo_fast_mode,
        "native hotkey must not publish into standalone Core configuration"
    );
    let saved = engine
        .capture_camera_continuation()
        .map_err(anyhow::Error::msg)?;
    engine.reset_replay_pacing_for_new_game();
    assert!(
        !engine.replay_fast_forward,
        "actual receiving new-game reset clears TiVO"
    );
    game_engine::common::global_data::write().tivo_fast_mode = true;
    engine.admit_camera_continuation(
        CnCGameEngine::decode_camera_continuation(&saved).map_err(anyhow::Error::msg)?,
    );
    assert!(engine.replay_fast_forward);
    assert!(game_engine::common::global_data::read().tivo_fast_mode);
    engine.toggle_replay_fast_forward_hotkey();
    assert!(!engine.replay_fast_forward);
    assert!(
        game_engine::common::global_data::read().tivo_fast_mode,
        "restored owner toggles without clearing a foreign Core flag"
    );
    game_engine::common::global_data::write().tivo_fast_mode = old_global;
    engine.host_match_in_replay = old_replay;

    let normal_30 = run_engine_replay_batch(&mut engine, false, 1.0 / 30.0, 1);
    assert_eq!(
        normal_30,
        (1, 1, 1),
        "normal 30 Hz engine frame must advance one queue/exit/fixed step"
    );

    let fast_30 = run_engine_replay_batch(&mut engine, true, 1.0 / 30.0, 1);
    assert_eq!(replay_logic_step_count(true), 4);
    assert_eq!(
        fast_30,
        (4, 4, 4),
        "TiVO fast-forward at 30 Hz must advance four queue/exit/fixed steps"
    );

    let normal_60 = run_engine_replay_batch(&mut engine, false, 1.0 / 60.0, 4);
    assert_eq!(
        normal_60,
        (2, 2, 2),
        "four normal 60 Hz engine frames must complete only two fixed steps"
    );

    let fast_60 = run_engine_replay_batch(&mut engine, true, 1.0 / 60.0, 1);
    assert_eq!(
        fast_60,
        (2, 2, 2),
        "four TiVO 60 Hz offers must advance owner queues only for the two completed fixed steps"
    );
    println!("Replay batches passed at 30 and 60 Hz with matching production observations");
    #[cfg(feature = "game_client")]
    source_command_phase_engine_witness(&mut engine);
    #[cfg(feature = "game_client")]
    if source_shell_load_engine_witness(&mut engine, startup_catalogs)? {
        shell_to_skirmish_longevity_witness(&mut engine)?;
        // This witness requires the actual admitted native Shell/map owner.
        // An unrequested licensed-content gate does not claim its coverage.
        actual_team_failure_stops_native_shell_and_fast_forward(&mut engine);
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn replay_factory_declares_queue_exit_before_counting_integer_delay() {
    let template = replay_factory_template();
    let metadata = template
        .production_exit_metadata
        .expect("authored Queue exit module");
    assert!(metadata.is_queue());
    assert_eq!(metadata.exit_delay_frames, 9);
    assert_eq!(metadata.initial_burst, 0);
    let mut object = crate::game_logic::Object::new(template, ObjectId(1), Team::USA);
    object.queue_exit_record(&metadata);
    object.tick_queue_exit(&metadata);
    assert_eq!(object.production_exit_state.delay_frames, 8);
}

#[cfg(test)]
#[test]
fn replay_fast_forward_runs_shadow_queue_per_actual_fixed_step() {
    // The full regression runs through the `internal` probe binary because
    // macOS requires winit event loops on the process main thread, while
    // Rust's unit-test harness executes tests on worker threads.
    assert_eq!(replay_logic_step_count(false), 1);
    assert_eq!(replay_logic_step_count(true), 4);
}
