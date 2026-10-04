//! CPP AIUpdate.cpp:197/238 owns victim and mood fields; 5012/5160 transfers
//! their raw state. These fixtures prove that bounded ownership boundary,
//! not complete inherited AI wire compatibility or whole-world isolation.

use super::UnitAIUpdate;
use crate::common::{Coord3D, ObjectID};
use crate::helpers::TheGameLogic;
use crate::modules::AIUpdateInterface;
use crate::object::Object;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};

#[path = "owned_ai_reset_tests.rs"]
mod reset_tests;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_OWNED_AI_STATE_CHILD",
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

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object OwnedAiFieldUnit\n KindOf = VEHICLE\n Behavior = AIUpdateInterface OwnedAiFields\n End\nEnd\n"
        ),
        1
    );
}

struct FactoryRuntime {
    // A fresh bounded child retains the genuinely classified factory Unit.
    // No UNIT_REGISTRY entry, fake machine or substitute cached AI is added.
    _factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
    id: ObjectID,
}

impl FactoryRuntime {
    fn new() -> Self {
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "OwnedAiFieldUnit",
                Coord3D::new(20.0, 30.0, 0.0),
                None,
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let unit = factory.get_object(id).unwrap();
        assert!(unit.is_unit());
        let owner = unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        assert!(super::registry::get_unit_arc(id).is_none());
        let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
        Self {
            _factory: factory,
            owner,
            ai,
            id,
        }
    }
}

struct RestoreAmbientFrame(u64);
impl RestoreAmbientFrame {
    fn set(frame: u64) -> Self {
        let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
        let previous = logic.get_current_frame();
        logic.set_current_frame(frame);
        Self(previous)
    }
}
impl Drop for RestoreAmbientFrame {
    fn drop(&mut self) {
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .set_current_frame(self.0);
    }
}

fn runtime(id: ObjectID) -> UnitAIUpdate {
    UnitAIUpdate::new(
        id,
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[test]
fn factory_ai_field_defaults_are_inert_and_ignore_ambient_clock() {
    if !child(concat!(
        module_path!(),
        "::factory_ai_field_defaults_are_inert_and_ignore_ambient_clock"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(83);
    let first = FactoryRuntime::new();
    let ai = first.ai.lock().unwrap();
    assert_eq!(ai.get_current_victim(), None);
    assert_eq!(
        ai.get_next_mood_check_time(),
        0,
        "CPP ctor initializes zero"
    );
}

#[test]
fn retained_factory_same_id_ai_fields_and_query_clones_are_independent() {
    if !child(concat!(
        module_path!(),
        "::retained_factory_same_id_ai_fields_and_query_clones_are_independent"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let first = FactoryRuntime::new();
    let second = FactoryRuntime::new();
    assert_eq!(
        first.id, second.id,
        "independent factories allocate the same ID"
    );
    assert!(!Arc::ptr_eq(&first.owner, &second.owner));
    assert!(!Arc::ptr_eq(&first.ai, &second.ai));
    let queried = first
        .owner
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    assert!(Arc::ptr_eq(&queried, &first.ai));
    {
        let mut ai = first.ai.lock().unwrap();
        ai.set_current_victim(Some(401));
        ai.set_next_mood_check_time(61);
    }
    {
        let mut ai = second.ai.lock().unwrap();
        ai.set_current_victim(Some(902));
        ai.set_next_mood_check_time(137);
    }
    assert_eq!(queried.lock().unwrap().get_current_victim(), Some(401));
    assert_eq!(queried.lock().unwrap().get_next_mood_check_time(), 61);
    assert_eq!(second.ai.lock().unwrap().get_current_victim(), Some(902));
    assert_eq!(second.ai.lock().unwrap().get_next_mood_check_time(), 137);
    {
        let mut ai = queried.lock().unwrap();
        // CPP transferAttack changes the raw victim without calling the
        // targeter setter; notifying attack goals does not change that field.
        ai.transfer_attack(401, 503);
        assert_eq!(ai.get_current_victim(), Some(503));
        ai.notify_new_victim_chosen(701);
        assert_eq!(ai.get_current_victim(), Some(503));
    }
    queried.lock().unwrap().set_current_victim(None);
    assert_eq!(first.ai.lock().unwrap().get_current_victim(), None);
    assert_eq!(second.ai.lock().unwrap().get_current_victim(), Some(902));
}

#[test]
fn cached_factory_ai_body_save_does_not_reborrow_locked_ambient_game_logic() {
    if !child(concat!(
        module_path!(),
        "::cached_factory_ai_body_save_does_not_reborrow_locked_ambient_game_logic"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    actual.ai.lock().unwrap().set_current_victim(Some(450));
    actual.ai.lock().unwrap().set_next_mood_check_time(91);
    let mut saved = Cursor::new(Vec::new());
    {
        // A bounded child turns the OLD ambient frame re-entry into a failure.
        // Release this guard before factory/Object teardown in the GREEN path.
        let foreign = crate::system::game_logic::get_game_logic().lock().unwrap();
        let mut ai = actual.ai.lock().unwrap();
        assert!(
            ai.xfer_ai_update_state(&mut XferSave::new(&mut saved, 1))
                .unwrap()
        );
        assert_eq!(ai.get_current_victim(), Some(450));
        assert_eq!(ai.get_next_mood_check_time(), 91);
        drop(ai);
        drop(foreign);
    }
    assert!(!saved.into_inner().is_empty());
}

#[test]
fn ai_body_restore_preserves_owned_victim_mood_and_serialized_jitter() {
    if !child(concat!(
        module_path!(),
        "::ai_body_restore_preserves_owned_victim_mood_and_serialized_jitter"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    // A real UnitAI value supplies the serialized jitter bit; neither a fake
    // machine nor a Unit registration is needed for these existing body fields.
    let mut saved = runtime(0xA1_5A_71);
    saved.set_current_victim(Some(647));
    saved.set_next_mood_check_time(241);
    saved.randomly_offset_mood_check = true;
    let mut bytes = Cursor::new(Vec::new());
    assert!(
        saved
            .xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
            .unwrap()
    );
    let loaded = FactoryRuntime::new();
    let mut ai = loaded.ai.lock().unwrap();
    ai.set_next_mood_check_time(7);
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(bytes.into_inner()), 1))
            .unwrap()
    );
    // CPP xfer reads nextMood DIRECTLY after jitter, not its public setter,
    // whose normal command contract explicitly clears jitter (4451–4455).
    assert!(
        ai.take_random_mood_offset(),
        "load must preserve original jitter bit"
    );
    assert!(!ai.take_random_mood_offset());
    assert_eq!(ai.get_current_victim(), Some(647));
    assert_eq!(ai.get_next_mood_check_time(), 241);
}

#[test]
fn cached_factory_ai_self_victim_clear_does_not_relock_own_interface() {
    if !child(concat!(
        module_path!(),
        "::cached_factory_ai_self_victim_clear_does_not_relock_own_interface"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.set_current_victim(Some(actual.id));
    assert_eq!(ai.get_current_victim(), Some(actual.id));
    // CPP AIUpdate.cpp:4173–4190 notifies the old target before clearing.
    // When the target is this exact runtime, notification must be local:
    // looking up and locking its cached AI again would deadlock this guard.
    ai.set_current_victim(None);
    assert_eq!(ai.get_current_victim(), None);
}

#[test]
fn cached_factory_update_admits_due_path_queue_and_consumes_timer() {
    if !child(concat!(
        module_path!(),
        "::cached_factory_update_admits_due_path_queue_and_consumes_timer"
    )) {
        return;
    }
    // AIUpdate.cpp:1047-1053 queues at the due frame and immediately clears
    // m_queueForPathFrame. Exercise that actual cached AI.update boundary,
    // rather than manually admitting an ID or installing a fake Unit handle.
    let _serial = crate::test_sync::lock();
    let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
    crate::system::engine_stores::with_active_stores(&stores, || {
        let _restore_frame = RestoreAmbientFrame::set(17);
        definitions();
        let runtime = FactoryRuntime::new();
        assert_ne!(runtime.id, crate::common::INVALID_ID);
        let pathfinder = stores.ai().read().unwrap().pathfinder().unwrap();
        let crc = || {
            let mut bytes = Vec::new();
            pathfinder
                .read()
                .unwrap()
                .crc_pathfinder(&mut XferSave::new(Cursor::new(&mut bytes), 1));
            bytes
        };
        let before = crc();
        runtime.ai.lock().unwrap().set_queue_for_path_time(1);
        runtime.ai.lock().unwrap().update().unwrap();
        assert_eq!(crc(), before, "future timer must not admit the actor");
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .set_current_frame(18);
        runtime.ai.lock().unwrap().update().unwrap();
        let admitted = crc();
        let ring_offset = 4 * 4 + 2 + 4 + 4;
        let ring_end = ring_offset + crate::ai::pathfind_complete::PATHFIND_QUEUE_LEN * 4;
        assert_eq!(
            &admitted[ring_offset..ring_offset + 4],
            &runtime.id.to_le_bytes(),
            "actual due cached runtime must enqueue its exact admitted owner ID"
        );
        assert_eq!(&admitted[ring_end..ring_end + 8], &[0, 0, 0, 0, 1, 0, 0, 0]);
        // Obtaining the same Pathfinder write after update also proves the
        // admission borrow has ended. Reset only this owned queue, then update
        // at the same frame: a consumed timer cannot re-admit the actor.
        pathfinder.write().unwrap().reset();
        let reset = crc();
        runtime.ai.lock().unwrap().update().unwrap();
        assert_eq!(crc(), reset, "due timer must clear after admission");
    });
}
