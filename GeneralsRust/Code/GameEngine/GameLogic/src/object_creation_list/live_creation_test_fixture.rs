//! Exact ownership of objects produced by the parser test's live OCL calls.

use crate::common::{GameError, ObjectID, ThingTemplate};
use crate::object::Object;
use crate::object_creation_list::{LiveThingFactoryContext, ThingFactoryContext};
use crate::object_manager::get_object_manager;
use crate::system::game_logic::get_game_logic;
use crate::team::Team;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, RwLock};

type Admission = (ObjectID, Arc<RwLock<Object>>);

pub(super) struct LiveCreationFixture {
    baseline: Vec<Admission>,
    created: RefCell<Vec<Admission>>,
    retired: std::cell::Cell<bool>,
    seed: [u32; 6],
}

impl LiveCreationFixture {
    pub(super) fn new() -> Result<Self, String> {
        let baseline = Self::admissions()?;
        Self::require_live_baseline(&baseline)?;
        Ok(Self {
            baseline,
            created: RefCell::new(Vec::new()),
            retired: std::cell::Cell::new(false),
            seed: game_engine::common::random_value::get_game_logic_random_seed_state(),
        })
    }

    fn admissions() -> Result<Vec<Admission>, String> {
        let logic = get_game_logic();
        let logic = logic
            .lock()
            .map_err(|_| "OCL fixture canonical owner poisoned".to_string())?;
        logic
            .get_all_object_ids()
            .iter()
            .map(|id| {
                logic
                    .find_object_by_id(*id)
                    .map(|object| (*id, object))
                    .ok_or_else(|| format!("OCL admission {id} has no canonical object"))
            })
            .collect()
    }

    fn require_live_baseline(baseline: &[Admission]) -> Result<(), String> {
        for (id, object) in baseline {
            let object = object
                .read()
                .map_err(|_| format!("unrelated OCL baseline object {id} poisoned"))?;
            // destroyObject sets DESTROYED before queuing processDestroyList.
            // Refuse to consume another fixture's pending destruction queue.
            if object.is_destroyed() {
                return Err(format!("unrelated object {id} has pending destruction"));
            }
        }
        Ok(())
    }

    fn record_admissions(&self) -> Result<(), String> {
        let current = Self::admissions()?;
        let mut created = self.created.borrow_mut();
        for (id, object) in current {
            if let Some((_, original)) = self.baseline.iter().find(|(old, _)| *old == id) {
                if !Arc::ptr_eq(original, &object) {
                    return Err(format!("unrelated OCL baseline identity {id} changed"));
                }
            } else if let Some((_, previous)) = created.iter().find(|(old, _)| *old == id) {
                if !Arc::ptr_eq(previous, &object) {
                    return Err(format!("OCL fixture identity {id} changed"));
                }
            } else {
                // An authored onCreate can invoke another live factory outside
                // this recorder. The serialized fixture owns those admissions,
                // too; record them before the next OCL nugget executes.
                created.push((id, object));
            }
        }
        Ok(())
    }

    pub(super) fn retire(&self) -> Result<(), String> {
        if self.retired.get() {
            return Ok(());
        }
        self.record_admissions()?;
        Self::require_live_baseline(&self.baseline)?;
        let created = self.created.borrow().clone();
        let owner = get_game_logic();
        {
            let mut owner = owner
                .lock()
                .map_err(|_| "OCL fixture canonical owner poisoned".to_string())?;
            let detached = {
                let manager = get_object_manager();
                let mut manager = manager
                    .write()
                    .map_err(|_| "OCL fixture object manager poisoned".to_string())?;
                created
                    .iter()
                    .map(|(id, object)| {
                        manager
                            .detach_fixture_object_slot(*id, object, &owner)
                            .map(|slot| (*id, slot, object.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            // C++ ThingFactory.cpp:281-301 admits every object into GameLogic.
            // OCL.cpp:1524-1535 returns only the first. Keep all pins alive and
            // release manager/object/recorder guards before destruction hooks.
            for (id, _) in &created {
                owner.destroy_object(*id);
            }
            owner
                .process_destroy_list()
                .map_err(|error| format!("OCL canonical retirement: {error:?}"))?;
            let manager = get_object_manager();
            let manager = manager
                .read()
                .map_err(|_| "OCL fixture object manager poisoned".to_string())?;
            for (id, slot, object) in detached {
                manager.finish_fixture_object_slot_retirement(id, slot, &object, &owner)?;
            }
        }
        let current = Self::admissions()?;
        if current.len() != self.baseline.len()
            || self.baseline.iter().any(|(id, object)| {
                !current
                    .iter()
                    .any(|(other, admitted)| id == other && Arc::ptr_eq(object, admitted))
            })
        {
            let ids: Vec<_> = current.iter().map(|(id, _)| *id).collect();
            return Err(format!(
                "unaccounted admissions after OCL retirement: {ids:?}"
            ));
        }
        Self::require_live_baseline(&self.baseline)?;
        crate::helpers::set_game_logic_random_seed(self.seed);
        self.retired.set(true);
        Ok(())
    }
}

impl ThingFactoryContext for LiveCreationFixture {
    fn find_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>> {
        LiveThingFactoryContext.find_template(name)
    }

    fn new_object(
        &self,
        template: Arc<dyn ThingTemplate>,
        team: &Arc<RwLock<Team>>,
    ) -> Result<Arc<RwLock<Object>>, GameError> {
        // No RefCell borrow crosses actual construction or recursive callbacks.
        let result = LiveThingFactoryContext.new_object(template, team);
        self.record_admissions().map_err(GameError::SystemError)?;
        if let Ok(object) = &result {
            let recorded = self
                .created
                .borrow()
                .iter()
                .any(|(_, admitted)| Arc::ptr_eq(object, admitted));
            if !recorded {
                return Err(GameError::SystemError(
                    "live OCL factory returned an unrecorded canonical owner".into(),
                ));
            }
        }
        result
    }
}

impl Drop for LiveCreationFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.retire().expect("exact live OCL fixture retirement");
        }));
        crate::helpers::set_game_logic_random_seed(self.seed);
        if let Err(error) = result {
            if unwinding {
                eprintln!("OCL fixture retirement failed during assertion unwind");
            } else {
                resume_unwind(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::AsciiString;
    use crate::object::registry::OBJECT_REGISTRY;

    struct ExistingAdmission(Admission);

    impl Drop for ExistingAdmission {
        fn drop(&mut self) {
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
                let owner = get_game_logic();
                let mut owner = owner
                    .lock()
                    .map_err(|_| "existing fixture owner poisoned".to_string())?;
                let admitted = owner
                    .find_object_by_id(self.0.0)
                    .ok_or_else(|| "existing fixture admission disappeared".to_string())?;
                if !Arc::ptr_eq(&admitted, &self.0.1) {
                    return Err("existing fixture identity changed".to_string());
                }
                owner.destroy_object(self.0.0);
                owner.process_destroy_list().map_err(|e| e.to_string())
            }));
            if std::thread::panicking() {
                if !matches!(result, Ok(Ok(()))) {
                    eprintln!("existing OCL fixture retirement failed during unwind");
                }
            } else {
                match result {
                    Ok(result) => result.expect("existing OCL fixture retirement"),
                    Err(error) => resume_unwind(error),
                }
            }
        }
    }

    fn exercise_retirement(unwind: bool) {
        let _guard = crate::test_sync::lock();
        let id = 0x0C1E_0001;
        let available = get_game_logic()
            .lock()
            .unwrap()
            .find_object_by_id(id)
            .is_none();
        assert!(available, "existing fixture must not replace another owner");
        let existing = Arc::new(RwLock::new(Object::new_test(id, 173.0)));
        let admission = get_game_logic()
            .lock()
            .unwrap()
            .register_object(existing.clone());
        admission.expect("unrelated canonical fixture admission");
        let existing = ExistingAdmission((id, existing));
        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("OclRetirementFixtureTeam"),
            0x0C1E_0002,
        )));
        let created = RefCell::new(Vec::<Arc<RwLock<Object>>>::new());
        let ids = RefCell::new(Vec::new());
        let seed = game_engine::common::random_value::get_game_logic_random_seed_state();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let fixture = LiveCreationFixture::new().expect("scoped live fixture");
            let template = fixture
                .find_template("AmericaInfantryRanger")
                .expect("real live factory Ranger definition");
            for _ in 0..2 {
                let object = fixture
                    .new_object(template.clone(), &team)
                    .expect("real live factory creation");
                let id = object.read().unwrap().get_id();
                ids.borrow_mut().push(id);
                created.borrow_mut().push(object);
            }
            if unwind {
                panic!("intentional OCL assertion unwind");
            }
            fixture.retire().expect("explicit exact retirement");
        }));
        assert_eq!(outcome.is_err(), unwind);
        assert_eq!(created.borrow().len(), 2);
        let canonical_facts = {
            let owner = get_game_logic();
            let owner = owner.lock().unwrap();
            (
                owner
                    .find_object_by_id(id)
                    .is_some_and(|object| Arc::ptr_eq(&object, &existing.0.1)),
                ids.borrow()
                    .iter()
                    .all(|id| owner.find_object_by_id(*id).is_none()),
            )
        };
        let manager_facts = {
            let manager = get_object_manager();
            let manager = manager.read().unwrap();
            ids.borrow()
                .iter()
                .all(|id| manager.get_object(*id).is_none())
        };
        assert!(canonical_facts.0, "unrelated canonical admission preserved");
        assert!(canonical_facts.1, "every created canonical owner retired");
        assert!(manager_facts, "every created manager slot retired");
        for id in ids.borrow().iter() {
            assert!(OBJECT_REGISTRY.get_object(*id).is_none());
        }
        let retired_ids: Vec<_> = created
            .borrow()
            .iter()
            .map(|object| object.read().unwrap().get_id())
            .collect();
        assert!(
            retired_ids
                .iter()
                .all(|id| *id == crate::common::INVALID_ID)
        );
        assert_eq!(
            game_engine::common::random_value::get_game_logic_random_seed_state(),
            seed,
            "fixture restores the unrelated simulation RNG"
        );
    }

    #[test]
    fn real_live_creation_retirement_preserves_existing_admission() {
        exercise_retirement(false);
    }

    #[test]
    fn real_live_creation_unwind_retires_all_objects() {
        exercise_retirement(true);
    }
}
