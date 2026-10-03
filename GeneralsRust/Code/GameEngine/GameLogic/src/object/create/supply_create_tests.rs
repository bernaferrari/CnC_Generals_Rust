//! Real supply create callbacks through Object::init_object (C++ Object.cpp).
use super::{SupplyCenterCreate, SupplyWarehouseCreate};
use crate::object::ObjectThingHandle;
use crate::object::update::ai_update::dozer_ai_update::construction_callback_tests::CompletionFixture;
use crate::player::{Player, player_list};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, Module, ModuleData, ModuleInterfaceType,
};
use std::sync::{Arc, RwLock};

// Supply create modules expose CreateInterface; this adapter lets the real
// Object module dispatch exercise them without changing production factories.
struct SupplyCreateModule<T> {
    create: T,
    data: Arc<BaseModuleData>,
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Module for SupplyCreateModule<T> {
    fn get_module_name_key(&self) -> u32 {
        crate::common::name_key_generate("SupplyCreateTest")
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(&self.create)
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Snapshotable
    for SupplyCreateModule<T>
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.create.load_post_process()
    }
}

fn install<T: CreateInterface + Snapshotable + Send + Sync + 'static>(
    fixture: &CompletionFixture,
    name: &str,
    create: T,
) {
    let data = Arc::new(BaseModuleData::new());
    fixture.structure.write().unwrap().install_module_for_test(
        name,
        Box::new(SupplyCreateModule {
            create,
            data: data.clone(),
        }),
        data,
        ModuleInterfaceType::CREATE,
    );
}

#[test]
fn warehouse_init_registers_with_all_resource_managers_without_relocking_owner() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.player.write().unwrap().init_from_dict_defaults();
    let other = Arc::new(RwLock::new(Player::new(1)));
    other.write().unwrap().init_from_dict_defaults();
    let uninitialized = Arc::new(RwLock::new(Player::new(2)));
    player_list().write().unwrap().add_player(other.clone());
    player_list()
        .write()
        .unwrap()
        .add_player(uninitialized.clone());
    // Map supply warehouses can be neutral: registration is not a controller operation.
    fixture.structure.write().unwrap().set_team(None);
    let thing = Arc::new(ObjectThingHandle::new(&fixture.structure));
    install(
        &fixture,
        "SupplyWarehouseCreate",
        SupplyWarehouseCreate::new(thing),
    );
    let id = fixture.structure.read().unwrap().get_id();
    for player in [&fixture.player, &other] {
        assert!(
            player
                .read()
                .unwrap()
                .get_resource_manager()
                .unwrap()
                .get_supply_warehouses()
                .is_empty()
        );
    }
    fixture.structure.write().unwrap().init_object().unwrap();
    for player in [&fixture.player, &other] {
        assert_eq!(
            player
                .read()
                .unwrap()
                .get_resource_manager()
                .unwrap()
                .get_supply_warehouses(),
            &[id]
        );
        assert!(
            player
                .read()
                .unwrap()
                .get_resource_manager()
                .unwrap()
                .get_supply_centers()
                .is_empty()
        );
    }
    assert!(
        uninitialized
            .read()
            .unwrap()
            .get_resource_manager()
            .is_none()
    );
    // Completing a warehouse must not register it again after removal:
    // C++ registers warehouses onCreate, supply centers onBuildComplete.
    for player in [&fixture.player, &other] {
        player
            .write()
            .unwrap()
            .get_resource_manager_mut()
            .unwrap()
            .remove_supply_warehouse(id);
    }
    fixture.structure.write().unwrap().on_build_complete();
    for player in [&fixture.player, &other] {
        assert!(
            player
                .read()
                .unwrap()
                .get_resource_manager()
                .unwrap()
                .get_supply_warehouses()
                .is_empty()
        );
    }
}

#[test]
fn center_init_waits_for_build_completion_before_registering() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.player.write().unwrap().init_from_dict_defaults();
    let thing = Arc::new(ObjectThingHandle::new(&fixture.structure));
    install(
        &fixture,
        "SupplyCenterCreate",
        SupplyCenterCreate::new(thing),
    );
    fixture.structure.write().unwrap().init_object().unwrap();
    assert!(
        fixture
            .player
            .read()
            .unwrap()
            .get_resource_manager()
            .unwrap()
            .get_supply_centers()
            .is_empty()
    );
    fixture.structure.write().unwrap().on_build_complete();
    assert_eq!(
        fixture
            .player
            .read()
            .unwrap()
            .get_resource_manager()
            .unwrap()
            .get_supply_centers(),
        &[fixture.structure.read().unwrap().get_id()]
    );
}

#[test]
fn object_without_controller_does_not_receive_init_difficulty_bonus() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.structure.write().unwrap().set_team(None);
    assert!(
        crate::scripting::engine::get_script_engine()
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .get_objects_should_receive_difficulty_bonus()
    );
    assert!(
        !fixture
            .structure
            .read()
            .unwrap()
            .is_receiving_difficulty_bonus()
    );
    fixture.structure.write().unwrap().init_object().unwrap();
    assert!(
        !fixture
            .structure
            .read()
            .unwrap()
            .is_receiving_difficulty_bonus(),
        "C++ Object::initObject applies scripted difficulty only inside the controller branch"
    );
}

#[test]
fn object_count_notification_records_frame_without_populating_name_cache() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let _fixture = CompletionFixture::new(0);
    let frame = crate::helpers::TheGameLogic::get_frame() as u32;
    let (changed_frame, cached_id) = {
        let engine = crate::scripting::engine::get_script_engine();
        let mut engine = engine.write().unwrap();
        let engine = engine.as_mut().unwrap();
        engine.set_frame_object_count_changed(frame.wrapping_add(1));
        engine.notify_of_object_creation_or_destruction();
        (
            engine.get_frame_object_count_changed(),
            crate::scripting::engine::get_named_object_tracker()
                .get_object_id("CompletionScriptName")
                .unwrap(),
        )
    };
    assert_eq!(
        changed_frame, frame,
        "C++ notification invalidates frame-keyed script condition results"
    );
    assert_eq!(
        cached_id, None,
        "C++ notification does not invoke createNamedCache"
    );
}

#[test]
fn controlled_object_receives_init_difficulty_bonus() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    assert!(
        !fixture
            .structure
            .read()
            .unwrap()
            .is_receiving_difficulty_bonus()
    );
    fixture.structure.write().unwrap().init_object().unwrap();
    assert!(
        fixture
            .structure
            .read()
            .unwrap()
            .is_receiving_difficulty_bonus()
    );
}

#[test]
fn first_script_update_populates_names_after_init_notification() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.structure.write().unwrap().init_object().unwrap();
    let tracker = crate::scripting::engine::get_named_object_tracker();
    assert_eq!(tracker.get_object_id("CompletionScriptName").unwrap(), None);
    crate::scripting::engine::get_script_engine()
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .update()
        .unwrap();
    assert_eq!(
        tracker.get_object_id("CompletionScriptName").unwrap(),
        Some(fixture.structure.read().unwrap().get_id())
    );
}

#[test]
fn destroy_notification_updates_frame_without_recaching_destroyed_owner() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let mut fixture = CompletionFixture::new(0);
    fixture.structure.write().unwrap().init_object().unwrap();
    let tracker = crate::scripting::engine::get_named_object_tracker();
    crate::scripting::engine::get_script_engine()
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .update()
        .unwrap();
    assert_eq!(
        tracker.get_object_id("CompletionScriptName").unwrap(),
        Some(fixture.structure.read().unwrap().get_id())
    );
    let frame = fixture.frame();
    crate::scripting::engine::get_script_engine()
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .set_frame_object_count_changed(frame.wrapping_add(1));
    let object_id = fixture.structure.read().unwrap().get_id();
    fixture.request_structure_destruction();
    assert_eq!(
        tracker.get_object_id("CompletionScriptName").unwrap(),
        Some(object_id),
        "C++ onDestroy leaves named-cache cleanup to the destructor"
    );
    let changed = crate::scripting::engine::get_script_engine()
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .get_frame_object_count_changed();
    assert_eq!(changed, frame.wrapping_add(1));
    fixture.finish_structure_destruction();
    assert_eq!(
        crate::scripting::engine::get_script_engine()
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .get_frame_object_count_changed(),
        frame
    );
    assert_eq!(tracker.get_object_id("CompletionScriptName").unwrap(), None);
}
