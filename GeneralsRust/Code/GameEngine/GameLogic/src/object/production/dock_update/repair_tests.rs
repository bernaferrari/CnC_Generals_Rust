//! RepairDockUpdate.cpp:87–128: cache the float rate, test completion before
//! healing, and heal a drone by its maximum HP only while repair continues.

use super::*;
use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};
use crate::object::body::body_module::BodyModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::system::game_logic::GameLogic;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Mutex;

const STATION: ObjectID = 0x7A3E_0001;
const DOCKER: ObjectID = 0x7A3E_0002;
const DRONE: ObjectID = 0x7A3E_0003;

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    let name = name.strip_prefix("gamelogic::").unwrap_or(name);
    matches!(
        crate::test_process::run_bounded(name, "GENERALS_REPAIR_DOCK_CHILD"),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

fn object_with_health(id: ObjectID, health: Real, max_health: Real) -> Arc<RwLock<Object>> {
    let mut object = Object::new_test(id, max_health);
    let data = ActiveBodyModuleData {
        initial_health: health,
        max_health,
        ..Default::default()
    };
    let body: Arc<Mutex<dyn BodyModuleInterface>> =
        Arc::new(Mutex::new(ActiveBody::new_with_owner(data, id)));
    object.set_body_module(Some(body));
    Arc::new(RwLock::new(object))
}

/// Both actual repair recipients are admitted to this local canonical owner.
/// Pins remain alive through owner-directed cleanup; no unrelated queue or
/// registry reset is consumed. The source station is deliberately absent:
/// C++ copies its ID into DamageInfo, without another station lookup.
struct RepairFixture {
    owner: GameLogic,
    docker: Arc<RwLock<Object>>,
    drone: Arc<RwLock<Object>>,
}

impl RepairFixture {
    fn new(docker_health: Real) -> Self {
        for id in [STATION, DOCKER, DRONE] {
            assert!(OBJECT_REGISTRY.get_object(id).is_none());
        }
        let mut fixture = Self {
            owner: GameLogic::new(),
            docker: object_with_health(DOCKER, docker_health, 100.0),
            drone: object_with_health(DRONE, 10.0, 25.0),
        };
        let docker_result = fixture.owner.register_object(Arc::clone(&fixture.docker));
        docker_result.expect("admit actual repair docker");
        let drone_result = fixture.owner.register_object(Arc::clone(&fixture.drone));
        drone_result.expect("admit actual repair drone");
        for (id, expected) in [(DOCKER, &fixture.docker), (DRONE, &fixture.drone)] {
            let canonical = fixture.owner.find_object_by_id(id).unwrap();
            let registered = OBJECT_REGISTRY.get_object(id).unwrap();
            assert!(Arc::ptr_eq(&canonical, expected));
            assert!(Arc::ptr_eq(&registered, expected));
        }
        assert!(OBJECT_REGISTRY.get_object(STATION).is_none());
        fixture
    }

    fn docker_health(&self) -> Real {
        self.docker.read().unwrap().get_health()
    }

    fn drone_health(&self) -> Real {
        self.drone.read().unwrap().get_health()
    }

    fn last_healing(object: &Arc<RwLock<Object>>) -> Option<DamageInfo> {
        let body = object.read().unwrap().get_body_module().unwrap();
        let info = body.lock().unwrap().get_last_damage_info();
        info
    }

    fn retire(&mut self) -> Result<(), String> {
        for (id, expected) in [(DOCKER, &self.docker), (DRONE, &self.drone)] {
            if let Some(actual) = self.owner.find_object_by_id(id) {
                if !Arc::ptr_eq(&actual, expected) {
                    return Err(format!("repair fixture canonical identity {id} changed"));
                }
                let registered = OBJECT_REGISTRY
                    .get_object(id)
                    .ok_or_else(|| format!("repair fixture registration {id} disappeared"))?;
                if !Arc::ptr_eq(&registered, expected) {
                    return Err(format!("repair fixture registry identity {id} changed"));
                }
                self.owner.destroy_object(id);
            }
        }
        self.owner
            .process_destroy_list()
            .map_err(|error| error.to_string())?;
        for (id, object) in [(DOCKER, &self.docker), (DRONE, &self.drone)] {
            if self.owner.find_object_by_id(id).is_some()
                || OBJECT_REGISTRY.get_object(id).is_some()
            {
                return Err(format!("repair fixture identity {id} survived retirement"));
            }
            let retired_id = object
                .read()
                .map_err(|_| "retired repair object poisoned")?
                .get_id();
            if retired_id != INVALID_ID {
                return Err(format!("repair fixture identity {id} was not finalized"));
            }
        }
        Ok(())
    }
}

impl Drop for RepairFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| self.retire()));
        if unwinding {
            if !matches!(result, Ok(Ok(()))) {
                eprintln!("exact repair fixture retirement failed during assertion unwind");
            }
        } else {
            match result {
                Ok(result) => result.expect("exact repair fixture retirement"),
                Err(error) => resume_unwind(error),
            }
        }
    }
}

fn assert_healing(info: DamageInfo, amount: Real) {
    assert_eq!(info.input.source_id, STATION);
    assert_eq!(info.input.damage_type, DamageType::Healing);
    assert_eq!(info.input.death_type, DeathType::None);
    assert_eq!(info.input.amount, amount);
}

#[test]
fn repair_dock_action_heals_drone_to_full_while_repair_continues() {
    if !child(concat!(
        module_path!(),
        "::repair_dock_action_heals_drone_to_full_while_repair_continues"
    )) {
        return;
    }
    let _isolation = crate::test_sync::lock();
    let fixture = RepairFixture::new(50.0);
    let data = RepairDockUpdateData {
        frames_for_full_heal: 10.0,
        ..Default::default()
    };
    let mut dock = RepairDockUpdate::new(data, STATION, &Coord3D::ZERO);
    assert!(dock.action(DOCKER, Some(DRONE)).expect("repair action"));
    assert_eq!(fixture.docker_health(), 55.0);
    assert_eq!(fixture.drone_health(), 25.0);
    assert_eq!(dock.last_repair, DOCKER);
    assert_eq!(dock.health_to_add_per_frame, 5.0);
    assert_healing(RepairFixture::last_healing(&fixture.docker).unwrap(), 5.0);
    assert_healing(RepairFixture::last_healing(&fixture.drone).unwrap(), 25.0);

    for tick in 2..=10 {
        assert!(dock.action(DOCKER, Some(DRONE)).expect("continued repair"));
        assert_eq!(fixture.docker_health(), 50.0 + tick as Real * 5.0);
        assert_eq!(dock.health_to_add_per_frame, 5.0);
    }
    assert_eq!(dock.last_repair, DOCKER, "the tenth action still docks");
    assert!(!dock.action(DOCKER, Some(DRONE)).expect("repair completion"));
    assert_eq!(dock.last_repair, INVALID_ID);
    assert_eq!(fixture.docker_health(), 100.0);
    assert_eq!(fixture.drone_health(), 25.0);
}

#[test]
fn repair_dock_action_leaves_drone_when_docker_repair_is_complete() {
    if !child(concat!(
        module_path!(),
        "::repair_dock_action_leaves_drone_when_docker_repair_is_complete"
    )) {
        return;
    }
    let _isolation = crate::test_sync::lock();
    let fixture = RepairFixture::new(100.0);
    let mut dock = RepairDockUpdate::new(RepairDockUpdateData::default(), STATION, &Coord3D::ZERO);
    assert!(!dock.action(DOCKER, Some(DRONE)).expect("complete docker"));
    assert_eq!(fixture.docker_health(), 100.0);
    assert_eq!(fixture.drone_health(), 10.0);
    assert_eq!(dock.last_repair, INVALID_ID);
    assert!(RepairFixture::last_healing(&fixture.docker).is_none());
    assert!(RepairFixture::last_healing(&fixture.drone).is_none());
}

#[test]
fn repair_dock_action_preserves_fractional_authored_frame_rate() {
    if !child(concat!(
        module_path!(),
        "::repair_dock_action_preserves_fractional_authored_frame_rate"
    )) {
        return;
    }
    let _isolation = crate::test_sync::lock();
    let fixture = RepairFixture::new(50.0);
    let data = RepairDockUpdateData {
        frames_for_full_heal: 0.5,
        ..Default::default()
    };
    let mut dock = RepairDockUpdate::new(data, STATION, &Coord3D::ZERO);
    assert!(dock.action(DOCKER, None).expect("fractional frame repair"));
    assert_eq!(dock.health_to_add_per_frame, 100.0);
    assert_eq!(fixture.docker_health(), 100.0);
    assert_eq!(dock.last_repair, DOCKER);
    assert_healing(RepairFixture::last_healing(&fixture.docker).unwrap(), 100.0);
    assert!(!dock.action(DOCKER, None).expect("fractional completion"));
    assert_eq!(dock.last_repair, INVALID_ID);
    assert_eq!(fixture.drone_health(), 10.0);
}

#[test]
fn dock_bone_start_indices_match_cpp() {
    assert_eq!(SINGLE_DOCK_BONE_START_INDEX, 0);
    assert_eq!(APPROACH_BONE_START_INDEX, 1);
}

#[test]
fn parse_time_for_full_heal_accepts_duration_suffixes() {
    let mut data = RepairDockUpdateData::default();
    let mut ini = INI::new();

    parse_time_for_full_heal(&mut ini, &mut data, &["1500ms"]).expect("duration");
    assert!((data.frames_for_full_heal - 45.0).abs() < f32::EPSILON);

    parse_time_for_full_heal(&mut ini, &mut data, &["1.5s"]).expect("duration");
    assert!((data.frames_for_full_heal - 45.0).abs() < f32::EPSILON);
}
