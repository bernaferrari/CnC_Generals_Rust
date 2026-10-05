//! C++ authored production-exit parsing and synchronous callback boundaries.

use super::DefaultProductionExitModuleData;
use crate::common::Coord3D;
use game_engine::common::ini::{INI, INIError};

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{ObjectID, PathfindLayerEnum};
use crate::modules::{AIUpdateInterface, ExitDoorType, ExitInterface};
use crate::object::Object;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, Mutex, RwLock};

#[test]
fn authored_exit_points_parse_cpp_axis_labels() {
    let mut data = DefaultProductionExitModuleData::default();
    INI::new()
        .with_inline_source(
            "UnitCreatePoint = X:10 Y:-25 Z:9\n NaturalRallyPoint = X:36 Y:-25 Z:3\n UseSpawnRallyPoint = Yes\n End",
            |ini| data.parse_from_ini(ini),
        )
        .expect("C++ DefaultProductionExit field table uses INI::parseCoord3D");
    assert_eq!(data.unit_create_point, Coord3D::new(10.0, -25.0, 9.0));
    assert_eq!(data.natural_rally_point, Coord3D::new(36.0, -25.0, 3.0));
    assert!(data.use_spawn_rally_point);
    assert!(matches!(
        INI::new().with_inline_source("UnknownExitField = 1\n End", |ini| data.parse_from_ini(ini)),
        Err(INIError::UnknownToken)
    ));
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_DEFAULT_EXIT_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

struct FactoryExit {
    _factory: ObjectFactory,
    producer: Arc<RwLock<Object>>,
    passenger: Arc<RwLock<Object>>,
}

impl FactoryExit {
    fn new() -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object AuthoredDefaultExitProducer\n KindOf = STRUCTURE\n Behavior = DefaultProductionExitUpdate Exit\n UnitCreatePoint = X:10 Y:-25 Z:9\n NaturalRallyPoint = X:36 Y:-25 Z:3\n End\nEnd\nObject AuthoredDefaultExitPassenger\n KindOf = INFANTRY\nEnd\n"
        ), 2);
        let mut factory = ObjectFactory::new();
        let producer_id = factory
            .create_object(
                "AuthoredDefaultExitProducer",
                Coord3D::new(100.0, 200.0, 20.0),
                None,
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let passenger_id = factory
            .create_object(
                "AuthoredDefaultExitPassenger",
                Coord3D::new(400.0, 500.0, 100.0),
                None,
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let producer = factory
            .get_object(producer_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        let passenger = factory
            .get_object(passenger_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        producer
            .write()
            .unwrap()
            .set_orientation(std::f32::consts::FRAC_PI_2)
            .unwrap();
        Self {
            _factory: factory,
            producer,
            passenger,
        }
    }

    fn exit(&self) {
        let mut exit = self
            .producer
            .read()
            .unwrap()
            .get_object_exit_interface()
            .expect("actual authored DefaultProductionExitUpdate interface");
        let id = self.passenger.read().unwrap().get_id();
        exit.exit_object_via_door(id, ExitDoorType::Primary)
            .unwrap();
    }

    fn natural_rally(&self) -> Coord3D {
        // C++ normalizes the authored full3D vector then adds two path cells
        // before applying the producer's model-to-world transform.
        let p = Coord3D::new(36.0, -25.0, 3.0);
        let scale = 2.0 * PATHFIND_CELL_SIZE_F / (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        self.producer
            .read()
            .unwrap()
            .get_transform_matrix()
            .transform_point3(Coord3D::new(
                p.x * (1.0 + scale),
                p.y * (1.0 + scale),
                p.z * (1.0 + scale),
            ))
    }
}

fn assert_coord_near(actual: Coord3D, expected: Coord3D) {
    assert!(
        (actual.x - expected.x).abs() < 0.001
            && (actual.y - expected.y).abs() < 0.001
            && (actual.z - expected.z).abs() < 0.001,
        "actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn authored_default_exit_places_unit_using_producer_transform_and_terrain() {
    if !child(concat!(
        module_path!(),
        "::authored_default_exit_places_unit_using_producer_transform_and_terrain"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = FactoryExit::new();
    assert!(
        fixture
            .passenger
            .read()
            .unwrap()
            .get_ai_update_interface()
            .is_none()
    );
    fixture.exit();
    // Unloaded terrain has height0; authored local Z9 plus producer Z20
    // must not survive the C++ layer-height placement override.
    let passenger = fixture.passenger.read().unwrap();
    assert_coord_near(*passenger.get_position(), Coord3D::new(125.0, 210.0, 0.0));
    assert_eq!(passenger.get_orientation(), std::f32::consts::FRAC_PI_2);
    assert_eq!(passenger.get_layer(), PathfindLayerEnum::Ground);
}

#[derive(Debug, PartialEq)]
enum Call {
    GroundQuery,
    Adjust,
    Follow(Vec<Coord3D>, Option<ObjectID>, CommandSourceType),
}

#[derive(Debug)]
struct CallbackWitness {
    owner: Arc<RwLock<Object>>,
    ground: bool,
    accepts_rally: bool,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl CallbackWitness {
    fn owner_released(&self) {
        assert!(
            self.owner.try_write().is_ok(),
            "production exit must release the passenger Object before synchronous AI callbacks"
        );
    }
}

impl AIUpdateInterface for CallbackWitness {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn is_doing_ground_movement(&self) -> bool {
        self.owner_released();
        self.calls.lock().unwrap().push(Call::GroundQuery);
        self.ground
    }
    fn adjust_destination(&mut self, rally: &mut Coord3D) -> bool {
        self.owner_released();
        self.calls.lock().unwrap().push(Call::Adjust);
        rally.x += 5.0;
        self.accepts_rally
    }
    fn execute_command(
        &mut self,
        command: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.owner_released();
        assert_eq!(command.cmd, AiCommandType::FollowExitProductionPath);
        self.calls.lock().unwrap().push(Call::Follow(
            command.coords.clone(),
            command.obj,
            command.cmd_source,
        ));
        Ok(())
    }
}

#[test]
fn authored_default_exit_releases_passenger_before_cpp_callback_sequence() {
    if !child(concat!(
        module_path!(),
        "::authored_default_exit_releases_passenger_before_cpp_callback_sequence"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = FactoryExit::new();
    let natural = fixture.natural_rally();
    let producer_id = fixture.producer.read().unwrap().get_id();
    // The installed producer is real; this AI explicitly witnesses callbacks,
    // not locomotor/scheduler execution by an actual cached UnitAI.
    for (has_rally, ground, accepts) in [
        (false, true, true),
        (true, true, true),
        (true, true, false),
        (true, false, true),
    ] {
        if has_rally {
            assert!(
                fixture
                    .producer
                    .write()
                    .unwrap()
                    .set_rally_point(&Coord3D::new(250.0, 300.0, 0.0))
            );
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        fixture
            .passenger
            .write()
            .unwrap()
            .set_ai_update_interface(Some(Arc::new(Mutex::new(CallbackWitness {
                owner: Arc::clone(&fixture.passenger),
                ground,
                accepts_rally: accepts,
                calls: Arc::clone(&calls),
            }))));
        fixture.exit();
        let mut expected = Vec::new();
        let mut path = vec![natural];
        if has_rally {
            expected.push(Call::GroundQuery);
            if ground {
                expected.push(Call::Adjust);
                if accepts {
                    path.push(Coord3D::new(255.0, 300.0, 0.0));
                }
            }
        }
        expected.push(Call::Follow(
            path,
            Some(producer_id),
            CommandSourceType::FromAi,
        ));
        let actual = calls.lock().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(&expected) {
            match (actual, expected) {
                (Call::Follow(a, a_owner, a_source), Call::Follow(b, b_owner, b_source)) => {
                    assert_eq!(a_owner, b_owner);
                    assert_eq!(a_source, b_source);
                    assert_eq!(a.len(), b.len());
                    for (&a, &b) in a.iter().zip(b) {
                        assert_coord_near(a, b);
                    }
                }
                _ => assert_eq!(actual, expected),
            }
        }
    }
}
