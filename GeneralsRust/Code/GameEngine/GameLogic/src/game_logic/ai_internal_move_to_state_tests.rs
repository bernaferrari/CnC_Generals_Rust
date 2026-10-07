use super::*;

use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
use crate::modules::AIUpdateInterface;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Default)]
struct TestAI {
    movement_targets: Vec<Coord3D>,
    adjust_destination: Vec<bool>,
    path_extra_distance: Vec<f32>,
    friend_ending_moves: usize,
    destroyed_paths: usize,
}

impl AIUpdateInterface for TestAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_moving(&self) -> bool {
        false
    }

    fn is_idle(&self) -> bool {
        true
    }

    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        self.movement_targets.push(*target);
        Ok(())
    }

    fn get_locomotor_distance_to_goal(&self) -> f32 {
        10.0
    }

    fn is_allowed_to_adjust_destination(&self) -> bool {
        true
    }

    fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjust_destination.push(adjust);
    }

    fn set_path_extra_distance(
        &mut self,
        distance: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.path_extra_distance.push(distance);
        Ok(())
    }

    fn friend_ending_move(&mut self) {
        self.friend_ending_moves += 1;
    }

    fn destroy_path(&mut self) {
        self.destroyed_paths += 1;
    }
}

struct RegisteredOwner {
    id: crate::common::ObjectID,
    _object: Arc<RwLock<Object>>,
    ai: Arc<Mutex<TestAI>>,
}

impl RegisteredOwner {
    fn new() -> Self {
        static NEXT_TEST_ID: AtomicU32 = AtomicU32::new(0x7f00_0000);
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let object = Object::new_with_id(
            Arc::new(DefaultThingTemplate::new(format!("MoveHelperTest{id}"))),
            id,
            ObjectStatusMaskType::none(),
            None,
        )
        .expect("create registered move-helper owner");
        let ai = Arc::new(Mutex::new(TestAI::default()));
        let erased_ai: Arc<Mutex<dyn AIUpdateInterface>> = ai.clone();
        object
            .write()
            .expect("owner write")
            .set_ai_update_interface(Some(erased_ai));
        Self {
            id,
            _object: object,
            ai,
        }
    }

    fn remove_ai(&self) {
        self._object
            .write()
            .expect("owner write")
            .set_ai_update_interface(None);
    }
}

impl Drop for RegisteredOwner {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.id);
    }
}

fn exercise_borrowed_helper() {
    let _serial = crate::test_sync::lock();
    let owner = RegisteredOwner::new();
    let mut helper = AIInternalMoveToState::new_with_owner_id(owner.id, "borrowed-test".into());
    let target = Coord3D::new(12.0, 34.0, 0.0);
    helper.set_goal_position(target);

    // This is the lock held by the production UnitAIUpdate::update caller.
    // The helper must use this borrow directly rather than reacquiring the
    // same AI Arc through its owner Object.
    let mut ai = owner.ai.lock().expect("installed AI lock");
    assert_eq!(
        helper.on_enter_with_ai(&mut *ai).expect("borrowed enter"),
        StateReturnType::Continue
    );
    assert_eq!(ai.movement_targets, vec![target]);
    assert_eq!(ai.path_extra_distance, vec![0.0]);
    assert_eq!(ai.adjust_destination, vec![true]);
    assert_eq!(
        helper.update_with_ai(&mut *ai).expect("borrowed update"),
        StateReturnType::Continue
    );
    helper
        .on_exit_with_ai(StateExitType::Normal, &mut *ai)
        .expect("borrowed exit");
    assert_eq!(ai.friend_ending_moves, 1);
    assert_eq!(ai.destroyed_paths, 1);
    drop(ai);

    // The legacy public API continues to resolve the installed AI handle when
    // the caller does not already own it.
    let mut legacy = AIInternalMoveToState::new_with_owner_id(owner.id, "legacy-test".into());
    legacy.set_goal_position(target);
    assert_eq!(
        legacy.on_enter().expect("legacy enter"),
        StateReturnType::Continue
    );
    assert_eq!(
        legacy.update().expect("legacy update"),
        StateReturnType::Continue
    );
    legacy.on_exit(StateExitType::Normal).expect("legacy exit");
}

#[test]
fn borrowed_callbacks_work_while_installed_ai_handle_is_held() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            "game_logic::ai_internal_move_to_state::tests::borrowed_callbacks_work_while_installed_ai_handle_is_held",
            "GENERALS_AI_MOVE_HELPER_BORROWED_CHILD",
        ) {
            return;
        }
    }
    exercise_borrowed_helper();
}

#[test]
fn legacy_enter_checks_immobile_before_missing_ai_and_exit_cleans_without_ai() {
    let _serial = crate::test_sync::lock();
    let owner = RegisteredOwner::new();
    owner.remove_ai();
    {
        let mut object = owner._object.write().expect("owner write");
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(ObjectStatusTypes::Immobile),
            true,
        );
        object.set_model_condition_state(ModelConditionFlags::MOVING);
    }
    let mut helper = AIInternalMoveToState::new_with_owner_id(owner.id, "no-ai-test".into());
    assert_eq!(
        helper.on_enter().expect("immobile enter"),
        StateReturnType::Failure
    );

    helper.ambient_playing_handle = u32::MAX;
    helper.on_exit(StateExitType::Normal).expect("no-AI exit");
    assert_eq!(helper.ambient_playing_handle, 0);
    if let Some(drawable) = owner._object.read().expect("owner read").get_drawable() {
        assert!(
            !drawable
                .read()
                .expect("drawable read")
                .get_model_conditions()
                .contains(ModelConditionFlags::MOVING),
            "exit without AI still clears the moving condition"
        );
    }
}
