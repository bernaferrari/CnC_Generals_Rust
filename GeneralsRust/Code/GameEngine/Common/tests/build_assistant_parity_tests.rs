use game_engine::common::system::build_assistant::{
    clear_build_assistant_backend, set_build_assistant_backend, BuildAssistant,
    BuildAssistantBackend, CanMakeType, Coord3D, LegalBuildCode, LocalLegalToBuildOptions, Object,
    ObjectID, Player, ThingTemplate,
};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

static BUILD_ASSISTANT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static FOOTPRINT_SAMPLES: OnceLock<Mutex<Vec<Coord3D>>> = OnceLock::new();

fn build_assistant_test_guard() -> MutexGuard<'static, ()> {
    BUILD_ASSISTANT_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("build assistant test lock")
}

#[test]
fn build_assistant_fails_closed_without_gamelogic_backend() {
    let _guard = build_assistant_test_guard();
    clear_build_assistant_backend();

    let assistant = BuildAssistant::new();
    let builder = Object {
        id: 7,
        position: Coord3D::new(0.0, 0.0, 0.0),
        orientation: 0.0,
    };
    let player = Player { player_index: 0 };
    let template = ThingTemplate::new("AmericaPowerPlant");
    let pos = Coord3D::new(64.0, 128.0, 0.0);

    assert_eq!(
        assistant.is_location_legal_to_build(
            &pos,
            &template,
            0.0,
            LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::CLEAR_PATH
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
            Some(&builder),
            Some(&player),
        ),
        LegalBuildCode::GenericFailure
    );
    assert!(assistant
        .build_object_now(Some(&builder), &template, &pos, 0.0, &player)
        .is_none());
    assert!(!assistant.is_possible_to_make_unit(&builder, &template));
    assert_eq!(
        assistant.can_make_unit(&builder, &template),
        CanMakeType::NoPrereq
    );
}

#[derive(Debug, Default)]
struct CapturingBackend {
    checks: Mutex<Vec<(u32, Option<ObjectID>)>>,
}

impl BuildAssistantBackend for CapturingBackend {
    fn build_object_now(
        &self,
        _builder_id: Option<ObjectID>,
        _template_name: &str,
        _pos: &Coord3D,
        _angle: f32,
        _owning_player: u32,
    ) -> Option<ObjectID> {
        None
    }

    fn is_location_legal_to_build(
        &self,
        _world_pos: &Coord3D,
        _template_name: &str,
        _angle: f32,
        options: LocalLegalToBuildOptions,
        builder_id: Option<ObjectID>,
        _player_id: Option<u32>,
    ) -> LegalBuildCode {
        self.checks
            .lock()
            .expect("checks lock")
            .push((options.bits(), builder_id));
        LegalBuildCode::Ok
    }

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        x + y * 0.5
    }
}

#[test]
fn tiled_locations_forward_cpp_line_build_flags_and_builder() {
    let _guard = build_assistant_test_guard();
    clear_build_assistant_backend();
    let backend = Arc::new(CapturingBackend::default());
    set_build_assistant_backend(backend.clone());

    let assistant = BuildAssistant::new();
    let builder = Object {
        id: 42,
        position: Coord3D::new(0.0, 0.0, 0.0),
        orientation: 0.0,
    };
    let template = ThingTemplate::new("ChinaWallSegment");
    let start = Coord3D::new(0.0, 0.0, 0.0);
    let end = Coord3D::new(30.0, 0.0, 0.0);

    let result = assistant
        .build_tiled_locations(&template, 0.0, &start, &end, 10.0, 10, Some(&builder))
        .expect("tile locations");

    assert_eq!(result.tiles_used, 4);
    assert_eq!(result.positions[0].z, 0.0);
    assert_eq!(result.positions[1].z, 10.0);
    assert_eq!(result.positions[2].z, 20.0);
    assert_eq!(result.positions[3].z, 30.0);
    let checks = backend.checks.lock().expect("checks lock");
    assert_eq!(checks.len(), 3);
    let expected = (LocalLegalToBuildOptions::USE_QUICK_PATHFIND
        | LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
        | LocalLegalToBuildOptions::CLEAR_PATH
        | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP
        | LocalLegalToBuildOptions::SHROUD_REVEALED)
        .bits();
    for (options, builder_id) in checks.iter() {
        assert_eq!(*options, expected);
        assert_eq!(*builder_id, Some(42));
    }

    clear_build_assistant_backend();
}

fn capture_footprint_sample(point: &Coord3D, user_data: &mut dyn std::any::Any) {
    let _ = user_data;
    FOOTPRINT_SAMPLES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("footprint samples lock")
        .push(*point);
}

#[test]
fn footprint_iteration_samples_backend_ground_height() {
    let _guard = build_assistant_test_guard();
    clear_build_assistant_backend();
    set_build_assistant_backend(Arc::new(CapturingBackend::default()));

    let assistant = BuildAssistant::new();
    let template = ThingTemplate::new("AmericaPowerPlant");
    let samples = FOOTPRINT_SAMPLES.get_or_init(|| Mutex::new(Vec::new()));
    samples.lock().expect("footprint samples lock").clear();
    let mut unused_user_data = ();

    assistant.iterate_footprint(
        &template,
        0.0,
        &Coord3D::new(100.0, 20.0, 0.0),
        20.0,
        capture_footprint_sample,
        &mut unused_user_data,
    );

    let samples = samples.lock().expect("footprint samples lock");
    assert!(!samples.is_empty());
    for sample in samples.iter() {
        assert_eq!(sample.z, sample.x + sample.y * 0.5);
    }

    clear_build_assistant_backend();
}
