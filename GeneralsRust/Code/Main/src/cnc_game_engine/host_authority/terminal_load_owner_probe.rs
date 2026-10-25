//! Actual main-thread engine and receiving physical-failure handler regression.
//! This executes the real handler, not a surrogate reset/state model. It does
//! not reproduce the saved-Drawable binding defect or claim a successful load.
use super::*;
use crate::game_logic::{
    GameLogic, NativeObjectRetirement, NativeViewEffects, Player, ScriptPresentationEffects, Team,
};

struct RejectSecond {
    calls: Vec<u32>,
}
impl NativeViewEffects for RejectSecond {
    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        // This recipient owns no native Drawable; no mixer admission occurs.
        let _ = (change, audio);
        Ok(false)
    }

    fn local_player_id(&self) -> u32 {
        1
    }
    fn host_epoch(&self) -> u64 {
        1
    }
    fn sample_slave(
        &mut self,
        _: &crate::game_logic::NativeSlaveSource,
        _: &str,
    ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
        panic!("retirement must not request camera sampling")
    }
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        panic!("the retirement receiver is loaned only after births")
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        let id = source.object.id;
        assert!(!source.live_objects.contains_key(&id));
        assert!(!source.live_objects.ordered_ids().contains(&id));
        self.calls.push(id.0);
        if self.calls.len() == 2 {
            Err("terminal-load actual receiving retirement rejection".into())
        } else {
            Ok(())
        }
    }
}

pub(crate) fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        event_loop.create_window(
            WindowAttributes::default()
                .with_title("terminal load owner regression")
                .with_visible(false),
        )?,
    );
    let private = std::env::var_os("GENERALS_TERMINAL_LOAD_PROBE_PRIVATE_ROOT")
        .ok_or_else(|| anyhow::anyhow!("requires private userdata launch root"))?;
    let args = Arc::new(CommandLineArgs::parse_from_args(vec![
        "terminal-load-owner-probe".into(),
        "--runtime_host=headless".into(),
        "--noaudio".into(),
        format!(
            "--userdata={}",
            std::path::PathBuf::from(private).join("userdata").display()
        ),
    ])?);
    let mut application_services = crate::subsystem_manager::ApplicationServices::new();
    let mut engine =
        pollster::block_on(CnCGameEngine::new(window, args, &mut application_services))?;
    // Startup progression is outside this handler regression. Invalidate its
    // existing worker before manually driving the real receiving Engine.
    engine.abandon_startup_load_worker();
    assert!(engine.runtime_host_headless);
    // Calibrate the ACTUAL UI staging path: a nonexistent save must preserve
    // each still-playable prior state and the actual receiving world clock.
    let missing = "terminal_owner_probe_intentionally_missing_slot";
    for prior in [GameState::InGame, GameState::Paused] {
        engine.game_logic.frame = 31;
        engine.current_state = prior;
        assert!(engine.game_logic.object_retirement_error().is_none());
        assert!(engine.host_load_game_from_ui(missing).is_err());
        assert_eq!(engine.current_state, prior);
        assert_eq!(engine.game_logic.get_frame(), 31);
    }
    // A healthy postcommit failure must still execute the actual normal reset
    // branch. This invokes the real handler with its typed committed outcome;
    // it does not fabricate a restore or inject an arbitrary world registry.
    engine.game_logic.frame = 47;
    engine.current_state = GameState::Loading;
    assert!(
        engine
            .host_handle_load_failure(
                missing,
                GameState::InGame,
                HostLoadFailure::Committed("recoverable postcommit calibration".into())
            )
            .is_err()
    );
    assert_eq!(engine.current_state, GameState::Menu);
    assert_eq!(engine.game_logic.get_frame(), 0);
    assert!(engine.game_logic.object_retirement_error().is_none());

    // Obtain a real parked request while the boundary is healthy. Retain that
    // source continuation until a later failure makes completion unavailable.
    assert!(
        engine.host_start_game_from_command_phase(HostStartRequest::without_player_template(
            GameMode::Skirmish,
            "USA".into(),
            "Lone Eagle".into(),
            None,
        ))
    );
    let deferred_before_failure = engine
        .pending_match_start
        .take()
        .expect("actual healthy park produced deferred start");
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("terminal-load-owner.ini".into(),
         "Object TerminalLoadOwner\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = DestroyDie DeleteTag\nEnd\nEnd\n".into())
    ]).map_err(anyhow::Error::msg)?).map_err(anyhow::Error::msg)?;
    world.add_player(Player::new(1, Team::USA, "TerminalOwner", true));
    let a = world
        .create_object("TerminalLoadOwner", Team::USA, glam::Vec3::ZERO)
        .expect("actual receiving content admits first actor");
    let b = world
        .create_object("TerminalLoadOwner", Team::USA, glam::Vec3::ONE)
        .expect("actual receiving content admits second actor");
    assert_ne!(a, b);
    world.frame = 73;
    let mut receiver = RejectSecond { calls: Vec::new() };
    let first_error = world
        .destroy_all_objects_immediate_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(&mut receiver),
            ..Default::default()
        })
        .unwrap_err()
        .to_string();
    assert_eq!(
        receiver.calls.len(),
        2,
        "one prior actor completed before terminal rejection"
    );
    assert!(first_error.contains("terminal-load actual receiving retirement rejection"));
    let failed = world
        .objects
        .failed_retirement_object()
        .expect("typed failure retains owner");
    let failed_id = failed.id;
    let generation = failed.visual_object_generation;
    let token = Arc::downgrade(&failed.visual_draw_identity_token);
    let retained_address = std::ptr::from_ref(failed);
    assert_ne!(generation, 0);
    assert!(!world.get_objects().lookup(&failed_id).is_some());
    assert!(world.object_retirement_error().is_some());
    engine.game_logic = world;
    let epoch = engine.host_direct_visual_world_epoch;
    engine.current_state = GameState::Loading;
    // Before the guard this invokes the ACTUAL Engine/client reset then panics
    // at GameLogic's terminal guard. No catch_unwind or error suppression.
    let result = engine.host_handle_load_failure(
        missing,
        GameState::InGame,
        HostLoadFailure::Committed(first_error.clone()),
    );
    assert_eq!(result.unwrap_err(), first_error);
    assert_eq!(engine.current_state, GameState::Exiting);
    assert!(engine.is_quitting());
    assert_eq!(engine.game_logic.get_frame(), 73, "no logical reset");
    assert_eq!(
        engine.host_direct_visual_world_epoch, epoch,
        "no reset epoch admission"
    );
    let retained = engine
        .game_logic
        .objects
        .failed_retirement_object()
        .unwrap();
    assert_eq!(std::ptr::from_ref(retained), retained_address);
    assert_eq!(retained.id, failed_id);
    assert_eq!(retained.visual_object_generation, generation);
    assert!(Arc::ptr_eq(
        &token.upgrade().unwrap(),
        &retained.visual_draw_identity_token
    ));
    // The typed failed-owner state, not an English error prefix, selects the
    // terminal route. A staging-labelled error cannot resume that same owner.
    engine.current_state = GameState::Loading;
    let again = engine.host_handle_load_failure(
        missing,
        GameState::Paused,
        HostLoadFailure::Staging("unrelated staged error wording".into()),
    );
    assert_eq!(again.unwrap_err(), "unrelated staged error wording");
    assert_eq!(engine.current_state, GameState::Exiting);
    assert_eq!(engine.game_logic.get_frame(), 73);
    let retry = engine
        .game_logic
        .destroy_all_objects_immediate_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(&mut receiver),
            ..Default::default()
        })
        .unwrap_err()
        .to_string();
    assert_eq!(retry, first_error);
    assert_eq!(
        receiver.calls.len(),
        2,
        "no callback replay in failure handling/retry"
    );
    assert_eq!(
        std::ptr::from_ref(
            engine
                .game_logic
                .objects
                .failed_retirement_object()
                .unwrap()
        ),
        retained_address
    );
    // Reuse the actual failed owner (not a fabricated failure reason) as a
    // detached retained pair. The native half is an empty epoch candidate:
    // this calibrates the real collection barrier, not BasicDrawable binding.
    let failed_world = std::mem::replace(&mut engine.game_logic, GameLogic::new());
    let detached_epoch = engine.next_native_world_epoch();
    engine.rejected_native_worlds.retain(
        Some(Box::new(failed_world)),
        crate::cnc_game_engine::map_native_admission::NativeMapCandidate::with_epoch(
            detached_epoch,
        ),
        first_error.clone(),
    );
    assert!(engine.rejected_native_worlds.validate_admission().is_err());
    assert!(
        token.upgrade().is_some(),
        "collection owns exact callback-failed actor"
    );
    engine.game_logic.frame = 89;
    assert!(engine.game_logic.object_retirement_error().is_none());
    engine.current_state = GameState::Loading;
    let staged = engine.host_handle_load_failure(
        missing,
        GameState::InGame,
        HostLoadFailure::Staging("detached candidate staging rejection".into()),
    );
    assert_eq!(staged.unwrap_err(), "detached candidate staging rejection");
    assert_eq!(
        engine.current_state,
        GameState::InGame,
        "healthy old world can continue despite future-admission barrier"
    );
    assert_eq!(engine.game_logic.get_frame(), 89);
    assert_eq!(engine.host_direct_visual_world_epoch, epoch);
    assert!(token.upgrade().is_some());
    assert_eq!(receiver.calls.len(), 2);
    engine.current_state = GameState::Loading;
    let committed = engine.host_handle_load_failure(
        missing,
        GameState::InGame,
        HostLoadFailure::Committed("detached candidate postcommit rejection".into()),
    );
    assert_eq!(
        committed.unwrap_err(),
        "detached candidate postcommit rejection"
    );
    assert_eq!(
        engine.current_state,
        GameState::Exiting,
        "collection barrier forbids reset after committed failure"
    );
    assert!(engine.is_quitting());
    assert_eq!(engine.game_logic.get_frame(), 89);
    assert_eq!(engine.host_direct_visual_world_epoch, epoch);
    assert!(engine.game_logic.object_retirement_error().is_none());
    assert!(engine.rejected_native_worlds.validate_admission().is_err());
    assert!(
        token.upgrade().is_some(),
        "failed candidate is not cleared by error handling"
    );
    assert_eq!(
        receiver.calls.len(),
        2,
        "no failed candidate callback replay"
    );
    println!(
        "PASS terminal load: actual UI calibration / retained installed and detached owners / no reset or retry"
    );
    // Add one actual healthy live actor, independent of the retained failed
    // same-ID world, so reset/admission cannot erase an empty fixture unnoticed.
    engine.game_logic.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "retained-boundary-live.ini".into(),
            "Object BoundaryLiveOwner\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
        )]).map_err(anyhow::Error::msg)?
    ).map_err(anyhow::Error::msg)?;
    engine
        .game_logic
        .add_player(Player::new(1, Team::USA, "BoundaryLive", true));
    let live_id = engine
        .game_logic
        .create_object("BoundaryLiveOwner", Team::USA, glam::Vec3::ZERO)
        .expect("healthy receiving old world remains independently usable");
    let live_token =
        Arc::downgrade(&engine.game_logic.objects[&live_id].visual_draw_identity_token);
    engine.selected_objects = vec![live_id];
    engine.runtime_host_ui_screen_override = Some("RetainedOwnerSentinel".into());
    let seed = engine.game_logic.logic_random.seed_words();
    let pending = bincode_legacy::serialize(&engine.game_logic.pending_object_removal_snapshot())
        .map_err(anyhow::Error::msg)?;
    let windows = engine
        .game_client
        .window_manager_owner()
        .with_manager_ref(|manager| manager.window_count());
    let verify = |engine: &CnCGameEngine| {
        assert_eq!(engine.current_state, GameState::Exiting);
        assert_eq!(engine.game_logic.get_frame(), 89);
        assert_eq!(engine.host_direct_visual_world_epoch, epoch);
        assert_eq!(engine.game_logic.logic_random.seed_words(), seed);
        assert_eq!(engine.game_logic.get_objects().registered_len(), 1);
        assert_eq!(engine.selected_objects, vec![live_id]);
        assert!(Arc::ptr_eq(
            &live_token.upgrade().unwrap(),
            &engine.game_logic.objects[&live_id].visual_draw_identity_token
        ));
        assert!(
            token.upgrade().is_some(),
            "retained failed pair remains owned"
        );
        assert!(engine.rejected_native_worlds.validate_admission().is_err());
        assert!(engine.game_logic.object_retirement_error().is_none());
        assert_eq!(
            bincode_legacy::serialize(&engine.game_logic.pending_object_removal_snapshot())
                .unwrap(),
            pending
        );
        assert_eq!(
            engine.runtime_host_ui_screen_override.as_deref(),
            Some("RetainedOwnerSentinel")
        );
        assert!(
            engine.pending_match_start.is_none(),
            "no fresh start candidate"
        );
        assert_eq!(
            engine
                .game_client
                .window_manager_owner()
                .with_manager_ref(|manager| manager.window_count()),
            windows,
            "no score/menu/loading layout admission on rejected InGame boundary"
        );
    };
    // Exercise the actual UI Quit-to-menu request, including its outer event
    // drain. A queued later event must not run after terminal handling.
    engine.current_state = GameState::InGame;
    engine
        .ui_manager
        .queue_event(crate::ui::UIEvent::ExitToMenu);
    engine
        .ui_manager
        .queue_event(crate::ui::UIEvent::TogglePause);
    engine.process_ui_events();
    verify(&engine);
    assert!(matches!(
        engine.ui_manager.pop_event(),
        Some(crate::ui::UIEvent::TogglePause)
    ));
    engine.current_state = GameState::InGame;
    engine
        .ui_manager
        .queue_event(crate::ui::UIEvent::StartGame {
            mode: GameMode::Skirmish,
            faction: "USA".into(),
            map: "Lone Eagle".into(),
            skirmish: None,
        });
    engine.process_ui_events();
    verify(&engine);
    // New-match park must reject before load-screen/seed/GUI side effects.
    engine.current_state = GameState::InGame;
    assert!(
        !engine.host_start_game_from_command_phase(HostStartRequest::without_player_template(
            GameMode::Skirmish,
            "USA".into(),
            "Lone Eagle".into(),
            None,
        ))
    );
    verify(&engine);
    // A healthy previously parked start must recheck ownership on completion.
    engine.current_state = GameState::InGame;
    engine.complete_parked_match_start(deferred_before_failure);
    verify(&engine);
    for score in [false, true] {
        engine.current_state = GameState::InGame;
        assert!(engine.clear_receiving_game_data(score).is_err());
        verify(&engine);
    }
    // Direct reset wrappers must return typed rejection before any callbacks.
    engine.current_state = GameState::InGame;
    assert!(matches!(
        engine.host_game_engine_reset(),
        Err(NativeBoundaryFailure::RetainedCandidate(_))
    ));
    verify(&engine);
    engine.current_state = GameState::InGame;
    assert!(engine.host_reset_for_admitted_replay_start().is_err());
    verify(&engine);
    engine.current_state = GameState::InGame;
    assert!(engine.host_reset_for_clear_game_data().is_err());
    verify(&engine);
    engine.current_state = GameState::InGame;
    assert!(engine.reset_match_state_after_engine_reset().is_err());
    verify(&engine);
    engine.current_state = GameState::InGame;
    assert!(engine.host_reset_game_logic().is_err());
    verify(&engine);
    // Actual Recorder ingress must not prepare/open a new file or mutate the
    // existing reader before rejecting the clear/admission boundary.
    let replay_file = engine
        .game_logic
        .replay_pending
        .recorder_owned()
        .get_current_replay_filename()
        .to_owned();
    engine.current_state = GameState::InGame;
    game_client::gui::callbacks::replay_menu::ingress::ReplayMenuReceiver::playback_file(
        &mut engine,
        "retained-boundary-must-not-open.rep".into(),
    );
    verify(&engine);
    assert_eq!(
        engine
            .game_logic
            .replay_pending
            .recorder_owned()
            .get_current_replay_filename(),
        replay_file
    );
    engine.current_state = GameState::InGame;
    game_client::gui::callbacks::replay_menu::ingress::ReplayMenuReceiver::restart(&mut engine);
    verify(&engine);
    assert_eq!(
        receiver.calls.len(),
        2,
        "no failed native destructor replay at later requests"
    );
    println!(
        "PASS retained candidate: actual Quit/menu/new-match/deferred/clear/reset/Recorder boundaries retain both owners"
    );

    Ok(())
}
