//! C++ OpenContain.cpp:987-998 / 1092-1103 refreshes idle vehicle owners
//! only when the departing rider has AI. These registered-object callbacks
//! witness that borrow boundary; they are not actual UnitAI or retail evidence.

use super::{OpenContain, OpenContainModuleData};
use crate::contain_module_overrides::ContainModuleDataAdapter;
use game_engine::common::thing::module::ModuleDataHandle;
use crate::common::{
    Coord3D, DefaultThingTemplate, ObjectID, ObjectStatusMaskType, PathfindLayerEnum,
};
use crate::modules::{AIUpdateInterface, ContainModuleInterface, ExitDoorType};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

type GoalCalls = Vec<(ObjectID, Coord3D)>;

#[derive(Debug)]
struct OwnerGoalAi {
    owner: Arc<RwLock<Object>>,
    idle: bool,
    calls: GoalCalls,
}

impl AIUpdateInterface for OwnerGoalAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        !self.idle
    }
    fn is_idle(&self) -> bool {
        self.idle
    }
    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: PathfindLayerEnum,
    ) -> Result<(), String> {
        let mut owner = self.owner.try_write().expect(
            "Open owner updateGoal callback must run after releasing its Object read borrow",
        );
        assert_eq!(
            *goal,
            *owner.get_position(),
            "C++ refreshes owner goal at owner position"
        );
        owner.set_destination_layer(layer);
        self.calls.push((owner.get_id(), *goal));
        Ok(())
    }
}

#[derive(Debug)]
struct RiderExitAi;

impl AIUpdateInterface for RiderExitAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        Ok(())
    }
}

fn registered_object(name: &str, id: ObjectID, kind: &str) -> Arc<RwLock<Object>> {
    let mut template = DefaultThingTemplate::new(name.to_string());
    template
        .parse_object_fields_from_ini(&HashMap::from([("KindOf".to_string(), kind.to_string())]))
        .expect("valid template fixture");
    Object::new_with_id(Arc::new(template), id, ObjectStatusMaskType::NONE, None).unwrap()
}

#[test]
fn owner_exit_goal_releases_object_borrow_and_preserves_cpp_gates() {
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::owner_exit_goal_releases_object_borrow_and_preserves_cpp_gates"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_OWNER_EXIT_GOAL_CHILD",
        ),
        crate::test_process::TestProcess::Child
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let owner_pos = Coord3D::new(20.0, 30.0, 40.0);
    for (mode, hurry) in [false, true].into_iter().enumerate() {
        for (case, (rider_has_ai, owner_idle, owner_vehicle)) in [
            (true, true, true),
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ]
        .into_iter()
        .enumerate()
        {
            let owner_id = 0x7B3D_1100 + (mode * 8 + case * 2) as ObjectID;
            let rider_id = owner_id + 1;
            let owner = registered_object(
                "OwnerGoalWitness",
                owner_id,
                if owner_vehicle {
                    "VEHICLE"
                } else {
                    "STRUCTURE"
                },
            );
            let rider = registered_object("RiderGoalWitness", rider_id, "INFANTRY");
            let owner_ai = Arc::new(Mutex::new(OwnerGoalAi {
                owner: Arc::clone(&owner),
                idle: owner_idle,
                calls: Vec::new(),
            }));
            {
                let mut owner = owner.write().unwrap();
                owner.set_position(&owner_pos).unwrap();
                owner.set_ai_update_interface(Some(owner_ai.clone()));
            }
            if rider_has_ai {
                let ai: Arc<Mutex<dyn AIUpdateInterface>> = Arc::new(Mutex::new(RiderExitAi));
                rider.write().unwrap().set_ai_update_interface(Some(ai));
            }
            let mut contain =
                OpenContain::new(Arc::downgrade(&owner), ModuleDataHandle::new(ContainModuleDataAdapter::new(OpenContainModuleData::default())))
                    .unwrap();
            ContainModuleInterface::contain_object(&mut contain, rider_id).unwrap();
            if hurry {
                contain.exit_object_in_a_hurry(rider_id).unwrap();
            } else {
                contain
                    .exit_object_via_door(rider_id, ExitDoorType::Door1)
                    .unwrap();
            }
            let expected = if rider_has_ai && owner_idle && owner_vehicle {
                vec![(owner_id, owner_pos)]
            } else {
                Vec::new()
            };
            assert_eq!(
                owner_ai.lock().unwrap().calls,
                expected,
                "hurry={hurry}, rider_ai={rider_has_ai}, idle={owner_idle}, vehicle={owner_vehicle}"
            );
            assert_eq!(contain.get_contain_count(), 0);
            assert_eq!(rider.read().unwrap().get_contained_by(), None);
            assert_eq!(*owner.read().unwrap().get_position(), owner_pos);
            owner.write().unwrap().set_ai_update_interface(None);
            OBJECT_REGISTRY.unregister_object(owner_id);
            OBJECT_REGISTRY.unregister_object(rider_id);
        }
    }
}

#[derive(Debug)]
struct ScatterBorrowWitness {
    rider: Arc<RwLock<Object>>,
    wrote_during_callback: bool,
    moved: bool,
}

impl AIUpdateInterface for ScatterBorrowWitness {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn ai_move_to_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.moved = true;
        self.wrote_during_callback = self.rider.try_write().is_ok();
        Ok(())
    }
}

#[test]
fn death_scatter_releases_object_write_before_ai_move() {
    // hq-on6u2 / OpenContain.cpp:679-709 — AI ignore/move must not run under
    // the passenger Object write that posed the scatter.
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::death_scatter_releases_object_write_before_ai_move"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_SCATTER_BORROW_CHILD",
        ),
        crate::test_process::TestProcess::Child
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    game_engine::common::common_services::with_common_services(&common, || {
        let core = crate::system::engine_stores::new_for_world();
        crate::system::engine_stores::with_active_stores(&core, || {
            let owner_id = 0x0A62_2001 as ObjectID;
            let rider_id = owner_id + 1;
            let owner = registered_object("ScatterOwner", owner_id, "VEHICLE");
            let rider = registered_object("ScatterRider", rider_id, "INFANTRY");
            owner
                .write()
                .unwrap()
                .set_position(&Coord3D::new(10.0, 20.0, 0.0));
            let witness = Arc::new(Mutex::new(ScatterBorrowWitness {
                rider: Arc::clone(&rider),
                wrote_during_callback: false,
                moved: false,
            }));
            rider.write().unwrap().set_ai_update_interface(Some(
                witness.clone() as Arc<Mutex<dyn AIUpdateInterface>>
            ));
            let contain = OpenContain::new(
                Arc::downgrade(&owner),
                ModuleDataHandle::new(ContainModuleDataAdapter::new(OpenContainModuleData::default())),
            )
            .unwrap();
            let followup = {
                let mut rider_guard = rider.write().unwrap();
                contain
                    .scatter_to_nearby_position(&mut rider_guard)
                    .expect("scatter plans under write")
            };
            let (ai, dest) = followup.expect("rider with AI returns follow-up");
            assert!(!witness.lock().unwrap().moved, "AI must wait for release");
            contain.apply_scatter_ai_followup(&ai, &dest);
            let w = witness.lock().unwrap();
            assert!(w.moved, "AI move runs after Object write release");
            assert!(
                w.wrote_during_callback,
                "callback must be able to reacquire passenger Object write"
            );
            drop(w);
            rider.write().unwrap().set_ai_update_interface(None);
            OBJECT_REGISTRY.unregister_object(owner_id);
            OBJECT_REGISTRY.unregister_object(rider_id);
        })
    });
}
