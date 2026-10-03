//! Actual named bridge/body transitions, with exact fixture lifetimes.
//! C++ ScriptConditions.cpp:242–270; TerrainLogic.cpp:852–909,1841–1899.
use super::leftover::{BridgeBrokenCondition, BridgeRepairedCondition};
use super::{ScriptCondition, ScriptContext, ScriptValue};
use crate::ai::{AI, the_ai};
use crate::common::{AsciiString, BodyDamageType, Coord3D, ObjectID};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::engine::get_named_object_tracker;
use crate::system::game_logic::GameLogic;
use crate::terrain::{BridgeInfo, TerrainLogic, get_terrain_logic};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, RwLock};
use std::time::Duration;

const BRIDGE_ID: ObjectID = 0x00B1_D6E0;
const VICTIM_ID: ObjectID = 0x00B1_D6E1;
const BRIDGE_NAME: &str = "RegistryBridgeDamageState";

struct BridgeConditionFixture<'guard> {
    _isolation: &'guard std::sync::MutexGuard<'static, ()>,
    owner: GameLogic,
    objects: Vec<Arc<RwLock<Object>>>,
    previous_terrain: Option<TerrainLogic>,
    previous_ai: Option<AI>,
    seed: [u32; 6],
}

impl<'guard> BridgeConditionFixture<'guard> {
    fn new(isolation: &'guard std::sync::MutexGuard<'static, ()>) -> Self {
        for id in [BRIDGE_ID, VICTIM_ID] {
            assert!(
                OBJECT_REGISTRY.get_object(id).is_none(),
                "fixture ID already admitted"
            );
        }
        assert!(
            get_named_object_tracker()
                .get_object_id(BRIDGE_NAME)
                .unwrap()
                .is_none()
        );
        let mut fixture = Self {
            _isolation: isolation,
            owner: GameLogic::new(),
            objects: Vec::new(),
            previous_terrain: None,
            previous_ai: None,
            seed: game_engine::common::random_value::get_game_logic_random_seed_state(),
        };
        // Bridge admission and damage change the pathfinder. Preserve its exact
        // prior AI owner together with the exact prior terrain, not a reset copy.
        fixture.previous_ai = Some(std::mem::replace(
            &mut *the_ai().write().unwrap(),
            AI::new(),
        ));
        fixture.previous_terrain = Some(std::mem::replace(
            &mut *get_terrain_logic().write().unwrap(),
            TerrainLogic::new(),
        ));
        fixture.owner.set_current_frame(37);
        for id in [BRIDGE_ID, VICTIM_ID] {
            let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
            fixture.owner.register_object(object.clone()).unwrap();
            fixture.objects.push(object);
        }
        fixture.objects[0]
            .write()
            .unwrap()
            .set_name(AsciiString::from(BRIDGE_NAME));
        get_named_object_tracker()
            .register_named_object(BRIDGE_NAME.into(), BRIDGE_ID)
            .unwrap();
        assert!(
            OBJECT_REGISTRY
                .get_object(BRIDGE_ID)
                .is_some_and(|object| Arc::ptr_eq(&object, &fixture.objects[0]))
        );

        let mut info = BridgeInfo::new();
        info.bridge_object_id = BRIDGE_ID;
        info.from = Coord3D::new(100.0, 100.0, 0.0);
        info.to = Coord3D::new(200.0, 100.0, 0.0);
        info.from_left = Coord3D::new(100.0, 90.0, 0.0);
        info.from_right = Coord3D::new(100.0, 110.0, 0.0);
        info.to_left = Coord3D::new(200.0, 90.0, 0.0);
        info.to_right = Coord3D::new(200.0, 110.0, 0.0);
        info.bridge_width = 20.0;
        get_terrain_logic()
            .write()
            .unwrap()
            .add_bridge_to_logic(info, "TestBridgeTemplate".into());
        let layer = get_terrain_logic()
            .read()
            .unwrap()
            .get_first_bridge()
            .unwrap()
            .get_layer();
        let mut victim = fixture.objects[1].write().unwrap();
        victim
            .set_position(&Coord3D::new(150.0, 100.0, 0.0))
            .unwrap();
        victim.set_layer(match layer as u8 {
            // Both engine enums preserve the original layer ordinals.
            2 => crate::common::PathfindLayerEnum::Top,
            other => panic!("fresh pathfinder expected first bridge layer, got {other}"),
        });
        drop(victim);
        fixture
    }

    fn set_bridge_body_state(&self, state: BodyDamageType) {
        let body = self.objects[0].read().unwrap().get_body_module().unwrap();
        let result = body.lock().unwrap().set_damage_state(state);
        result.unwrap();
        let actual = body.lock().unwrap().get_damage_state();
        assert_eq!(
            actual, state,
            "actual ActiveBody state drives bridge transitions"
        );
    }

    fn update_bridge_phase(&self) {
        // No Object/body guard spans the real immediate PF/falling-damage phase.
        get_terrain_logic()
            .write()
            .unwrap()
            .update_bridge_damage_states();
    }

    fn bridge_is_destroyed(&self) -> bool {
        let layer = get_terrain_logic()
            .read()
            .unwrap()
            .get_first_bridge()
            .unwrap()
            .get_layer();
        let pathfinder = the_ai().read().unwrap().pathfinder().unwrap();
        let destroyed = pathfinder.read().unwrap().bridge_is_destroyed(layer);
        destroyed.expect("registered fixture bridge layer")
    }

    fn retire(&mut self) -> Result<(), String> {
        for object in &self.objects {
            let id = object
                .read()
                .map_err(|_| "fixture Object poisoned")?
                .get_id();
            let admitted = self
                .owner
                .find_object_by_id(id)
                .ok_or("fixture admission missing")?;
            if !Arc::ptr_eq(&admitted, object) {
                return Err("fixture owner changed".into());
            }
            self.owner.destroy_object(id);
        }
        self.owner
            .process_destroy_list()
            .map_err(|error| error.to_string())?;
        get_named_object_tracker()
            .unregister_object(BRIDGE_ID)
            .map_err(|error| error.to_string())?;
        for id in [BRIDGE_ID, VICTIM_ID] {
            if self.owner.find_object_by_id(id).is_some()
                || OBJECT_REGISTRY.get_object(id).is_some()
            {
                return Err(format!("fixture object {id} remains admitted"));
            }
        }
        for object in &self.objects {
            if object
                .read()
                .map_err(|_| "retired Object poisoned")?
                .get_id()
                != crate::common::INVALID_ID
            {
                return Err("retired fixture object ID not invalidated".into());
            }
        }
        if get_named_object_tracker()
            .get_object_id(BRIDGE_NAME)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("fixture named bridge remains registered".into());
        }
        Ok(())
    }
}

impl Drop for BridgeConditionFixture<'_> {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.retire().expect("exact bridge retirement")
        }));
        // Restore aliases' actual prior contents even if a fixture assertion fails.
        if let Some(previous) = self.previous_terrain.take() {
            *get_terrain_logic()
                .write()
                .unwrap_or_else(|e| e.into_inner()) = previous;
        }
        if let Some(previous) = self.previous_ai.take() {
            *the_ai().write().unwrap_or_else(|e| e.into_inner()) = previous;
        }
        crate::helpers::set_game_logic_random_seed(self.seed);
        if let Err(error) = result {
            if unwinding {
                eprintln!("bridge fixture retirement failed while unwinding");
            } else {
                resume_unwind(error);
            }
        }
    }
}

async fn bridge_condition_scenario() {
    let isolation = crate::test_sync::lock();
    verify_unwind_restores_exact_inputs(&isolation);
    let fixture = BridgeConditionFixture::new(&isolation);
    assert!(!fixture.bridge_is_destroyed());
    let global_frame = crate::helpers::TheGameLogic::get_frame();
    let context = ScriptContext {
        game_time: Duration::ZERO,
        active_player: None,
        variables: HashMap::new(),
        game_state: crate::scripting::GameStateContext {
            map_name: "Test".into(),
            game_mode: "Test".into(),
            players: vec![],
            objectives: vec![],
        },
        host_trigger_world: Arc::new(std::sync::Mutex::new(Default::default())),
    };
    let params = HashMap::from([(
        "bridge_name".into(),
        ScriptValue::String(BRIDGE_NAME.into()),
    )]);
    // A named admitted object alone does not bypass the changed-state cache.
    assert!(
        !BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    assert!(
        !BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    fixture.set_bridge_body_state(BodyDamageType::Rubble);
    assert!(
        !BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    fixture.update_bridge_phase();
    assert!(fixture.bridge_is_destroyed());
    assert!(
        BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .expect("broken condition")
    );
    assert!(
        !BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .expect("repaired condition")
    );
    // C++ delivers DAMAGE_FALLING immediately in this phase, not next tick.
    let victim_health = fixture.objects[1].read().unwrap().get_health();
    assert_eq!(victim_health, 0.0);
    fixture.update_bridge_phase();
    assert!(
        !BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    assert!(
        !BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    fixture.set_bridge_body_state(BodyDamageType::Damaged);
    assert!(
        !BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    fixture.update_bridge_phase();
    assert!(!fixture.bridge_is_destroyed());
    assert!(
        !BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .expect("broken condition after repair")
    );
    assert!(
        BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .expect("repaired condition after repair")
    );
    fixture.update_bridge_phase();
    assert!(
        !BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .unwrap()
    );
    assert_eq!(fixture.owner.get_frame(), 37);
    assert_eq!(crate::helpers::TheGameLogic::get_frame(), global_frame);
    drop(fixture);
    assert!(OBJECT_REGISTRY.get_object(BRIDGE_ID).is_none());
    assert!(OBJECT_REGISTRY.get_object(VICTIM_ID).is_none());
    assert!(
        get_named_object_tracker()
            .get_object_id(BRIDGE_NAME)
            .unwrap()
            .is_none()
    );
}

fn verify_unwind_restores_exact_inputs(isolation: &std::sync::MutexGuard<'static, ()>) {
    let previous_pathfinder = the_ai().read().unwrap().pathfinder().unwrap();
    let previous_bridges = {
        let terrain = get_terrain_logic().read().unwrap();
        let mut ids = Vec::new();
        terrain.for_each_bridge(|bridge| ids.push(bridge.get_bridge_info().bridge_object_id));
        ids
    };
    let previous_height = get_terrain_logic()
        .read()
        .unwrap()
        .get_ground_height(150.0, 100.0, None);
    let previous_seed = game_engine::common::random_value::get_game_logic_random_seed_state();
    let previous_names = get_named_object_tracker().get_all_named_objects().unwrap();
    let mut pins = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let fixture = BridgeConditionFixture::new(isolation);
        pins = fixture.objects.clone();
        panic!("intentional bridge fixture assertion unwind");
    }));
    assert!(result.is_err());
    for (id, pin) in [BRIDGE_ID, VICTIM_ID].into_iter().zip(pins) {
        assert!(OBJECT_REGISTRY.get_object(id).is_none());
        assert_eq!(pin.read().unwrap().get_id(), crate::common::INVALID_ID);
    }
    let restored_pathfinder = the_ai().read().unwrap().pathfinder().unwrap();
    assert!(Arc::ptr_eq(&previous_pathfinder, &restored_pathfinder));
    let mut restored_bridges = Vec::new();
    get_terrain_logic()
        .read()
        .unwrap()
        .for_each_bridge(|bridge| restored_bridges.push(bridge.get_bridge_info().bridge_object_id));
    assert_eq!(restored_bridges, previous_bridges);
    assert_eq!(
        get_terrain_logic()
            .read()
            .unwrap()
            .get_ground_height(150.0, 100.0, None),
        previous_height
    );
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        previous_seed
    );
    let mut previous_names = previous_names;
    previous_names.sort();
    let mut restored_names = get_named_object_tracker().get_all_named_objects().unwrap();
    restored_names.sort();
    assert_eq!(restored_names, previous_names);
    // NamedObjectTracker intentionally retains didUnitExist history. This
    // fixture removes its exact live mapping; it does not reset global history.
}

pub(super) async fn run_bounded_bridge_condition_scenario() {
    #[cfg(not(target_arch = "wasm32"))]
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        "scripting::conditions::tests::bridge_conditions_use_terrain_bridge_damage_state",
        "GENERALS_NAMED_BRIDGE_CONDITION_CHILD",
    ) {
        return;
    }
    bridge_condition_scenario().await;
}
