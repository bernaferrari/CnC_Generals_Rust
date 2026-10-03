//! Genuine owner for the custom controller's tests, not a C++ cadence oracle.
//!
//! Production authored factories use object::behavior::StealthDetectorUpdate.
//! That C++ module has an owned Object and scheduled DetectionRate sleeps
//! (StealthDetectorUpdate.cpp:64-70,123-137,400), not this helper's countdown.

use super::{StealthDetectorController, StealthDetectorUpdate, StealthDetectorUpdateModuleData};
use crate::common::{INVALID_ID, ObjectID};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::system::game_logic::GameLogic;
use game_engine::common::thing::module::ModuleInterfaceType;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, RwLock};

pub(crate) struct DetectorFixture {
    owner: GameLogic,
    object: Arc<RwLock<Object>>,
    controller: Arc<Mutex<StealthDetectorController>>,
    id: ObjectID,
}

impl DetectorFixture {
    pub(crate) fn new(data: Arc<StealthDetectorUpdateModuleData>, id: ObjectID) -> Self {
        assert!(
            OBJECT_REGISTRY.get_object(id).is_none(),
            "detector ID already owned"
        );
        let module = StealthDetectorUpdate::new(0, data.clone(), id);
        let controller = module.get_controller();
        let mut object = Object::new_test(id, 100.0);
        object.install_module_for_test(
            "StealthDetectorUpdate",
            Box::new(module),
            data,
            ModuleInterfaceType::NONE,
        );
        let object = Arc::new(RwLock::new(object));
        let mut fixture = Self {
            owner: GameLogic::new(),
            object,
            controller,
            id,
        };
        let admission = fixture.owner.register_object(fixture.object.clone());
        admission.expect("owned detector admission");
        // Local GameLogic::register_object publishes this exact handle in the
        // registry. live_count is therefore nonzero; an unrelated object is
        // never needed to pass the custom controller's legacy world gate.
        assert!(
            OBJECT_REGISTRY
                .get_object(id)
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &fixture.object))
        );
        fixture
    }

    pub(crate) fn with_controller<R>(
        &self,
        f: impl FnOnce(&mut StealthDetectorController) -> R,
    ) -> R {
        let mut controller = self.controller.lock().expect("installed controller");
        f(&mut controller)
    }

    fn retire(&mut self) -> Result<(), String> {
        let admitted = self
            .owner
            .find_object_by_id(self.id)
            .ok_or_else(|| "detector canonical admission missing".to_string())?;
        if !Arc::ptr_eq(&admitted, &self.object) {
            return Err("detector canonical identity changed".into());
        }
        // Neither object nor controller guards span onDestroy/onDelete.
        self.owner.destroy_object(self.id);
        self.owner
            .process_destroy_list()
            .map_err(|e| e.to_string())?;
        if self.owner.find_object_by_id(self.id).is_some()
            || OBJECT_REGISTRY.get_object(self.id).is_some()
        {
            return Err("detector remains admitted after retirement".into());
        }
        let id = self
            .object
            .read()
            .map_err(|_| "retired detector object poisoned".to_string())?
            .get_id();
        if id != INVALID_ID {
            return Err("detector finalization did not invalidate its ID".into());
        }
        Ok(())
    }
}

impl Drop for DetectorFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.retire().expect("exact detector owner retirement");
        }));
        if let Err(error) = result {
            if unwinding {
                eprintln!("detector fixture retirement failed during assertion unwind");
            } else {
                resume_unwind(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise_retirement(unwind: bool) {
        let _guard = crate::test_sync::lock();
        let unrelated = DetectorFixture::new(Arc::new(Default::default()), 0x5D37_0003);
        let id = 0x5D37_0004;
        let mut pin = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let fixture = DetectorFixture::new(Arc::new(Default::default()), id);
            pin = Some(fixture.object.clone());
            if unwind {
                panic!("intentional detector fixture assertion unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(OBJECT_REGISTRY.get_object(id).is_none());
        let retired_id = pin.unwrap().read().unwrap().get_id();
        assert_eq!(retired_id, INVALID_ID);
        assert!(
            OBJECT_REGISTRY
                .get_object(unrelated.id)
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &unrelated.object))
        );
        let still_live = unrelated.object.read().unwrap().get_id();
        assert_eq!(still_live, unrelated.id);
    }

    #[test]
    fn installed_detector_normal_retirement_preserves_other_owner() {
        exercise_retirement(false);
    }

    #[test]
    fn installed_detector_unwind_retires_exact_owner() {
        exercise_retirement(true);
    }
}
