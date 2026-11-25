#[cfg(test)]
mod continuation_tests {
    //! Save/load continuation acceptance tests (audit deliverable).
    //!
    //! Property under test: "Run to frame N, save, continue N+K recording
    //! observations; restore the saved state, continue with identical inputs,
    //! compare the next K frames."
    //!
    //! Each branch drives a local GameLogic and lends its GameLogic and TeamFactory
    //! to the same GameState file codec. The receiving branch starts with its
    //! own authored template catalog, neutral roster, and saved ID high-water marks.
    //!
    //! Known save-format gap pinned here: save files do NOT carry the game-logic
    //! RNG seed words (`xfer_game_logic_state` has no seed xfer; mission loads
    //! hardcode `init_random_with_seed(0)`). Continuation parity therefore
    //! REQUIRES an explicit post-load reseed with the 6-word ADC state captured
    //! at save time. Test 1 proves parity WITH the reseed; test 2 is the negative
    //! control proving the harness DETECTS the gap when the reseed is omitted.
    //!
    //! Per-frame RNG consumer: the harness draws one logic-RNG value per frame
    //! and folds it into object positions, so the RNG stream is provably part
    //! of future behavior even though this script-light world consumes no draws
    //! inside `GameLogic::update()` itself.
    //!
    //! This bounded scenario drives identical RNG-backed position changes in
    //! both branches. Player-command and authored mission continuation require
    //! separate integration evidence.

    use super::tests::test_state_lock;
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use game_engine::common::random_value::{
        get_game_logic_random_seed_state, set_game_logic_random_seed_state,
    };
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use game_engine::System::SaveGame::BorrowedSnapshotBlock;
    use game_engine::{AvailableGameInfo, SaveCode, SaveFileType, SaveGameInfo, SnapshotType};

    use super::*;

    /// Frames executed before the save point.
    const SETUP_FRAMES: usize = 12;
    /// Frames executed after the save point in each branch.
    const CONTINUATION_FRAMES: usize = 10;

    /// (id, x, y) for the deterministic world. Ids are stable sort keys.
    const TEST_OBJECTS: &[(ObjectID, f32, f32)] = &[
        (0x2101, 10.0, 10.0),
        (0x2102, 40.0, 20.0),
        (0x2103, 70.0, 30.0),
    ];

    /// One frame of observable state: frame counter, RNG seed CRC, object
    /// count, sorted (id, health, x, y, z) tuples, and the harness draw that
    /// advanced the logic RNG stream during this frame.
    #[derive(Debug, Clone, PartialEq)]
    struct FrameTrace {
        frame: UnsignedInt,
        rng_crc: u32,
        object_count: usize,
        objects: Vec<(ObjectID, f32, f32, f32, f32)>,
        draw: i32,
    }

    /// Name the first differing field between two frame observations.
    fn describe_first_difference(a: &FrameTrace, b: &FrameTrace) -> Option<String> {
        if a.frame != b.frame {
            return Some(format!("frame counter {} != {}", a.frame, b.frame));
        }
        if a.rng_crc != b.rng_crc {
            return Some(format!(
                "rng seed crc {:#010x} != {:#010x}",
                a.rng_crc, b.rng_crc
            ));
        }
        if a.object_count != b.object_count {
            return Some(format!(
                "object count {} != {}",
                a.object_count, b.object_count
            ));
        }
        if a.objects != b.objects {
            for (i, (x, y)) in a.objects.iter().zip(b.objects.iter()).enumerate() {
                if x != y {
                    return Some(format!("object[{}] (id,health,pos) {:?} != {:?}", i, x, y));
                }
            }
            return Some("object lists differ in length".to_string());
        }
        if a.draw != b.draw {
            return Some(format!("logic rng draw {} != {}", a.draw, b.draw));
        }
        None
    }

    fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "generals_continuation_{}_{}_{}_{:?}",
            tag,
            std::process::id(),
            SETUP_FRAMES,
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create continuation save dir");
        dir
    }

    /// Register "TestObject" in the shared Common ThingFactory so the
    /// GameState load path can rebuild saved objects by template name
    /// (crate-standard `ensure_template_exists` pattern).
    fn ensure_test_object_template() {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template("TestObject", false).is_none() {
                factory.new_template("TestObject");
            }
        }
    }

    fn continuation_logic() -> (GameLogic, Arc<RwLock<crate::team::Team>>) {
        let owner = Arc::new(EngineStores::new_for_world());
        let team = with_active_stores(&owner, || {
            ensure_test_object_template();
            continuation_team()
        });
        admit_fixture_neutral_player(owner.services());
        (GameLogic::new_with_engine_stores(owner), team)
    }

    /// Object.cpp:4101-4111 restores an Object only onto a team the
    /// TeamFactory already admitted (SC_INVALID_DATA otherwise), so every
    /// continuation object belongs to one admitted team with a stable id.
    fn continuation_team() -> std::sync::Arc<std::sync::RwLock<crate::team::Team>> {
        const CONTINUATION_TEAM_ID: crate::team::TeamID = 91_401;
        let factory = crate::team::get_team_factory();
        let mut teams = factory.lock().unwrap();
        if let Some(team) = teams.find_team_by_id(CONTINUATION_TEAM_ID) {
            return team;
        }
        let prototype = teams
            .init_team_owned("ContinuationTeam".into(), "".into(), false, None, None)
            .unwrap();
        teams
            .create_team_on_prototype_with_id(&prototype, CONTINUATION_TEAM_ID)
            .unwrap()
    }

    /// Advance one frame, consume one logic-RNG draw from the harness, fold
    /// the draw into a stable (id-sorted) object's position, and observe.
    ///
    /// Stepping happens on the caller's LOCAL instance, mirroring
    /// `update_game_logic`'s `get_frame() -> update()` pattern without
    /// holding the singleton (which can block on the_ai / script globals).
    fn step_one_frame(logic: &mut GameLogic, rel: usize) -> FrameTrace {
        let frame = logic.get_frame();
        logic.update(frame, || None).expect("local GameLogic tick");

        // Guaranteed logic-RNG consumer: the save format does not carry the
        // seed words, so this draw makes the RNG stream provably part of the
        // observable continuation.
        let draw = crate::helpers::get_game_logic_random_value(0, 1000);

        // DEADLOCK NOTE: set_position drives the area tracker synchronously
        // (object_triggers.rs:255-268) which can re-enter TheGameLogic — it
        // must NEVER run while this thread holds the GAME_LOGIC mutex.
        // Selection keys off sorted ids (all_objects order
        // reverses across a save/load round-trip, prepend semantics).
        let mut ids: Vec<ObjectID> = logic.objects.keys().copied().collect();
        ids.sort_unstable();
        let nudge_target = if ids.is_empty() {
            None
        } else {
            logic.objects.get(&ids[rel % ids.len()]).cloned()
        };
        if let Some(arc) = nudge_target {
            let mut obj = arc.write().expect("object write lock");
            let pos = obj.get_position();
            let (x, y, z) = (pos.x, pos.y, pos.z);
            let nudge_x = ((draw % 7) as f32 - 3.0) * 0.5;
            let nudge_y = (((draw / 7) % 5) as f32 - 2.0) * 0.5;
            let _ = obj.set_position(&Coord3D::new(x + nudge_x, y + nudge_y, z));
        }

        let objects = ids
            .iter()
            .map(|id| {
                let arc = logic
                    .objects
                    .get(id)
                    .expect("sorted id must exist in object map");
                let obj = arc.read().expect("object read lock");
                let pos = obj.get_position();
                (*id, obj.get_health(), pos.x, pos.y, pos.z)
            })
            .collect();

        FrameTrace {
            frame: logic.get_frame(),
            rng_crc: crate::helpers::get_game_logic_random_seed_crc(),
            object_count: logic.get_object_count(),
            objects,
            draw,
        }
    }

    struct Scenario {
        trace_uninterrupted: Vec<FrameTrace>,
        trace_restored: Vec<FrameTrace>,
    }

    /// Save branch A, continue it, then replay from the file into branch B.
    fn run_continuation_scenario(tag: &str, reseed_after_load: bool) -> Scenario {
        let _ = crate::scripting::engine::initialize_script_engine();
        OBJECT_REGISTRY.clear();
        player_list().write().unwrap().clear();

        let (mut a, team) = continuation_logic();
        let owner = a.pathfinding_owner.as_ref().unwrap().clone();
        with_active_stores(&owner, || {
            let template = crate::helpers::TheThingFactory::find_template("TestObject").unwrap();
            for &(id, x, y) in TEST_OBJECTS {
                let actor = Object::new_with_id(
                    template.clone(),
                    id,
                    ObjectStatusMaskType::none(),
                    Some(team.clone()),
                )
                .unwrap();
                actor.write().unwrap().set_position(&Coord3D::new(x, y, 0.0)).unwrap();
                a.register_object(actor).unwrap();
            }
        });
        // This bounded snapshot contract exercises clock, objects and RNG
        // continuation. Player command and retail-content coverage are separate.

        // --- run to frame N ---
        for rel in 0..SETUP_FRAMES {
            let _ = step_one_frame(&mut a, rel + 1);
        }

        // --- save + snapshot RNG state ---
        // Redirect the global GameState save directory to a unique temp dir
        // so the acceptance run never writes into the working tree.
        // init_game_state preserves the
        // registered snapshot blocks (reset_for_init) and points the save
        // directory at the temp dir the save is written to.
        let save_dir = unique_temp_dir(tag);
        // Real (tiny) map file + global map_name: GameStateMap embeds the
        // pristine map on first save; a mapless synthetic world makes
        // embed_* fail with Error. first_save path copies these bytes.
        let map_path = save_dir.join("continuation_test.map");
        std::fs::write(&map_path, b"CONTINUATION TEST MAP\x00\x01\x02").expect("write dummy map");
        if game_engine::common::ini::get_global_data().is_none() {
            // GlobalData is a None-until-INI OnceCell; tests never parse INI,
            // so construct the default instance explicitly.
            game_engine::common::ini::init_global_data();
        }
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().map_name = map_path.to_string_lossy().to_string();
        }
        let save_directory = save_dir.join("Save");
        std::fs::create_dir_all(&save_directory).expect("create save directory");
        game_engine::System::init_game_state(save_directory);
        assert_eq!(a.get_frame(), SETUP_FRAMES as UnsignedInt);
        assert_eq!(a.get_object_count(), TEST_OBJECTS.len());
        let high_object_id = a.get_object_id_counter();
        let high_drawable_id = owner
            .services()
            .session_flags()
            .next_drawable_id
            .load(Ordering::Relaxed);
        let save_filename = format!("{}_continuation.sav", tag);
        {
            let mut state = game_engine::System::get_game_state();
            let mut teams = TeamFactorySnapshotBridge(owner.services().team_factory().clone());
            let mut campaign = Default::default();
            let mut logic = CampaignGameLogicSnapshot {
                logic: &mut a,
                campaign: &mut campaign,
            };
            let code = state
                .save_game_with_snapshots(
                    save_filename.clone(),
                    "continuation acceptance".to_string(),
                    SaveFileType::Normal,
                    SnapshotType::SaveLoad,
                    &mut [
                        BorrowedSnapshotBlock {
                            block_name: "CHUNK_GameLogic",
                            snapshot: &mut logic,
                        },
                        BorrowedSnapshotBlock {
                            block_name: "CHUNK_TeamFactory",
                            snapshot: &mut teams,
                        },
                    ],
                )
                .expect("save_game xfer must not fail");
            assert_eq!(code, SaveCode::Ok, "save_game must report SaveCode::Ok");
        }
        let saved_words = get_game_logic_random_seed_state();
        let saved_crc = crate::helpers::get_game_logic_random_seed_crc();

        // --- branch A: uninterrupted continuation ---
        let mut trace_uninterrupted = Vec::with_capacity(CONTINUATION_FRAMES);
        for rel in 0..CONTINUATION_FRAMES {
            trace_uninterrupted.push(step_one_frame(&mut a, rel));
        }

        // --- restore ---
        reset_game_logic().expect("reset_game_logic before load");
        // The GameLogic block loads only into an operation owner that also
        // receives the Campaign chunk (GameState.cpp loadGame -> xferSnapshot
        // of TheGameLogic after TheCampaignManager). Lend a fresh LOCAL
        // instance as that owner; the restored world lands directly in it.
        let (mut b, _) = continuation_logic();
        // This fixture lends GameLogic and TeamFactory separately from
        // GameStateMap. Seed the counters the map block normally restores.
        b.set_object_id_counter(high_object_id);
        let receiving = b.owning_world_services().unwrap();
        receiving
            .session_flags()
            .next_drawable_id
            .store(high_drawable_id, Ordering::Relaxed);
        {
            let receiving_teams = receiving.team_factory().clone();
            let mut team_snapshot = TeamFactorySnapshotBridge(receiving_teams);
            let mut campaign = game_engine::System::CampaignManagerXferState::default();
            let mut owner = CampaignGameLogicSnapshot {
                logic: &mut b,
                campaign: &mut campaign,
            };
            let mut state = game_engine::System::get_game_state();
            let code = state
                .load_game_with_snapshots(
                    AvailableGameInfo {
                        filename: save_filename.clone(),
                        save_game_info: SaveGameInfo::default(),
                    },
                    &mut [
                        BorrowedSnapshotBlock {
                            block_name: "CHUNK_GameLogic",
                            snapshot: &mut owner,
                        },
                        BorrowedSnapshotBlock {
                            block_name: "CHUNK_TeamFactory",
                            snapshot: &mut team_snapshot,
                        },
                    ],
                )
                .expect("load_game xfer must not fail");
            assert_eq!(code, SaveCode::Ok, "load_game must report SaveCode::Ok");
        }
        assert_eq!(
            b.get_frame(),
            SETUP_FRAMES as UnsignedInt,
            "restore must rewind the frame counter to the save point"
        );
        assert_eq!(
            b.get_object_count(),
            TEST_OBJECTS.len(),
            "restore must rebuild every saved object"
        );
        if reseed_after_load {
            set_game_logic_random_seed_state(saved_words);
            assert_eq!(
                crate::helpers::get_game_logic_random_seed_crc(),
                saved_crc,
                "post-load reseed must restore the save-point RNG CRC"
            );
        }

        // --- branch B: replay the same K frames with identical inputs ---
        let mut trace_restored = Vec::with_capacity(CONTINUATION_FRAMES);
        for rel in 0..CONTINUATION_FRAMES {
            trace_restored.push(step_one_frame(&mut b, rel));
        }

        // --- cleanup ---
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().map_name.clear();
        }
        let _ = std::fs::remove_dir_all(&save_dir);
        reset_game_logic().expect("reset_game_logic after scenario");
        OBJECT_REGISTRY.clear();

        Scenario {
            trace_uninterrupted,
            trace_restored,
        }
    }

    /// Positive test: with the post-load RNG reseed, the restored run must be
    /// indistinguishable from the uninterrupted run, frame by frame.
    #[test]
    fn save_load_continuation_matches_uninterrupted_execution() {
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                concat!(
                    module_path!(),
                    "::save_load_continuation_matches_uninterrupted_execution"
                )
                .strip_prefix("gamelogic::")
                .unwrap(),
                "GENERALS_SAVE_CONTINUATION_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        let _crate_lock = crate::test_sync::lock();
        let _state_lock = test_state_lock();

        let scenario = run_continuation_scenario("reseed", true);

        assert_eq!(scenario.trace_uninterrupted.len(), CONTINUATION_FRAMES);
        assert_eq!(scenario.trace_restored.len(), CONTINUATION_FRAMES);

        for (rel, (a, b)) in scenario
            .trace_uninterrupted
            .iter()
            .zip(scenario.trace_restored.iter())
            .enumerate()
        {
            if let Some(field) = describe_first_difference(a, b) {
                panic!(
                    "continuation diverged at relative frame {} (absolute frame {}): {}\n\
                     uninterrupted: {:?}\n  restored:      {:?}",
                    rel, a.frame, field, a, b
                );
            }
        }

        // The traces must actually exercise the observables: the draw values
        // prove the RNG stream advanced every frame in both branches.
        assert!(
            scenario.trace_uninterrupted.iter().any(|t| t.draw != 0),
            "uninterrupted branch must consume logic RNG draws"
        );
    }

    /// Negative control: without the post-load reseed, the continuation MUST
    /// diverge within K frames, because the save format does not serialize
    /// the game-logic RNG seed words. If this test ever fails, the save
    /// format gained seed serialization and the harness reseed is obsolete.
    #[test]
    fn continuation_without_rng_reseed_diverges() {
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                concat!(module_path!(), "::continuation_without_rng_reseed_diverges")
                    .strip_prefix("gamelogic::")
                    .unwrap(),
                "GENERALS_SAVE_CONTINUATION_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        let _crate_lock = crate::test_sync::lock();
        let _state_lock = test_state_lock();

        let scenario = run_continuation_scenario("noreseed", false);

        let mut first_divergence: Option<(usize, String)> = None;
        for (rel, (a, b)) in scenario
            .trace_uninterrupted
            .iter()
            .zip(scenario.trace_restored.iter())
            .enumerate()
        {
            if let Some(field) = describe_first_difference(a, b) {
                first_divergence = Some((rel, field));
                break;
            }
        }

        let (rel, field) = first_divergence.unwrap_or_else(|| {
            panic!(
                "continuation without RNG reseed must diverge: save files do not carry the \
                 logic RNG seed words (documented save-format gap); got identical traces for \
                 all {} frames",
                CONTINUATION_FRAMES
            )
        });
        assert!(
            rel < CONTINUATION_FRAMES,
            "divergence must occur within the K continuation frames"
        );
        // The divergence signature must be RNG-backed: differing draws or
        // seed CRCs, not merely an unrelated bookkeeping field.
        assert!(
            field.contains("rng") || field.contains("draw"),
            "expected an RNG-driven divergence signature, got: {}",
            field
        );
    }

    #[test]
    fn reset_preserves_loaded_template_identity() {
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                concat!(module_path!(), "::reset_preserves_loaded_template_identity")
                    .strip_prefix("gamelogic::")
                    .unwrap(),
                "GENERALS_SAVE_RESET_CATALOG_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        let _crate_lock = crate::test_sync::lock();
        let _state_lock = test_state_lock();
        let mut logic = GameLogic::new();
        logic.init();
        ensure_test_object_template();
        let before = get_thing_factory()
            .unwrap()
            .as_ref()
            .unwrap()
            .find_template("TestObject", true)
            .unwrap();
        logic.reset();
        let after = get_thing_factory()
            .unwrap()
            .as_ref()
            .unwrap()
            .find_template("TestObject", true)
            .expect("reset retains authored template");
        assert!(
            std::sync::Arc::ptr_eq(&before, &after),
            "reset must retain the loaded definition, not reload or rebuild it"
        );
    }

    #[test]
    /// Regression (hq-ccble hazard 2): objects built from Common factory
    /// templates must survive GameLogic::update. Fixed by owner-explicit
    /// helper updates (object/helper/*): the helpers used to re-find their
    /// OWN owner through TheGameLogic::find_object_by_id, whose global
    /// first-touch chain never returned in local-instance contexts.
    fn factory_template_objects_survive_update() {
        let _crate_lock = crate::test_sync::lock();
        let _state_lock = test_state_lock();
        OBJECT_REGISTRY.clear();
        ensure_test_object_template();
        let template = crate::helpers::TheThingFactory::find_template("TestObject").unwrap();
        let mut logic = GameLogic::new();
        for &(id, _x, _y) in TEST_OBJECTS {
            let arc = Object::new_with_id(template.clone(), id, ObjectStatusMaskType::none(), Some(continuation_team()))
                .expect("create");
            logic.objects.insert(id, arc.clone());
            logic.all_objects.push(id);
        }
        for f in 0..3 {
            logic.update(f, || None).expect("tick");
        }
    }
}
