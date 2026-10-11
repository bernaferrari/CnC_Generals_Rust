//! Actual saved Overlord redirection and installed authored child interface.
use super::*;
use crate::modules::{ContainAdmissionTarget, contain_admission_with_lookup};
use crate::object::contain::{OverlordContain, OverlordContainModuleData};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::OnceLock;
fn raw(name: &str, id: ObjectID) -> Object {
    static DEFINITIONS: OnceLock<()> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        if get_thing_factory().unwrap().is_none() { init_thing_factory().unwrap(); }
        if get_module_factory().unwrap().is_none() { init_module_factory().unwrap(); }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let mut text = String::from("Object ThirdOwnerHull\n KindOf = VEHICLE\nEnd\nObject ThirdOwnerInfantry\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject ThirdOwnerVehicle\n KindOf = VEHICLE\n TransportSlotCount = 1\nEnd\n");
        for (name, slots) in [("ThirdOwnerBunker",2),("ThirdOwnerFull",0)] {
            text.push_str(&format!("Object {name}\n KindOf = VEHICLE\n Behavior = TransportContain Cargo\n Slots = {slots}\n AllowInsideKindOf = INFANTRY\n AllowNeutralInside = Yes\n End\nEnd\n"));
        }
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&text),5);
    });
    Object::new_raw(
        crate::helpers::TheThingFactory::find_template(name).unwrap(),
        id,
        crate::common::types::ObjectStatusMaskType::NONE,
        None,
    )
}
fn installed(name: &str) -> Arc<RwLock<Object>> {
    let child = Arc::new(RwLock::new(raw(name, 7312)));
    let template = crate::helpers::TheThingFactory::find_template(name).unwrap();
    Object::init_modules_for(&child, template.as_ref()).unwrap();
    child.write().unwrap().contained_by_id = 7311;
    child
}
fn redirected(hull: &Arc<RwLock<Object>>) -> OverlordContain {
    let mut definition = OverlordContainModuleData::default();
    definition.base.slot_capacity = 1;
    definition.base.base.allow_inside_kind_of = KindOf::Vehicle.cpp_mask();
    let mut module = OverlordContain::new(Arc::downgrade(hull), &definition).unwrap();
    let mut saved = module.save_state().unwrap();
    saved.insert("redirection_activated".into(), vec![1]);
    module.load_state(&saved).unwrap();
    module.base.base.contained_object_ids.push(7312);
    module
}
#[test]
fn supplied_child_admission_uses_same_id_owner_and_saved_redirection() {
    let hull = Arc::new(RwLock::new(raw("ThirdOwnerHull", 7311)));
    let peer = Arc::new(RwLock::new(raw("ThirdOwnerHull", 7311)));
    let child = installed("ThirdOwnerBunker");
    let full = installed("ThirdOwnerFull");
    let passenger = raw("ThirdOwnerInfantry", 7313);
    let wrong = raw("ThirdOwnerVehicle", 7314);
    let mut module = redirected(&hull);
    assert_eq!(
        module.admission_target(),
        ContainAdmissionTarget::ActiveFirstChild(7312)
    );
    let mut bytes = Vec::new();
    module
        .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    let mut restored = redirected(&peer);
    restored
        .snapshot_xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    // Resolve the saved literal payload into the driving store without publication.
    restored.base.base.contained_object_ids.clear();
    restored.base.base.contained_object_ids.push(7312);
    let module: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(module));
    let restored: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(restored));
    let owner = hull.write().unwrap();
    let peer_owner = peer.write().unwrap();
    assert!(contain_admission_with_lookup(
        &module,
        &passenger,
        &owner,
        true,
        &|_| Some(Arc::clone(&child))
    ));
    assert!(!contain_admission_with_lookup(
        &module,
        &wrong,
        &owner,
        true,
        &|_| Some(Arc::clone(&child))
    ));
    assert!(!contain_admission_with_lookup(
        &restored,
        &passenger,
        &peer_owner,
        true,
        &|_| Some(Arc::clone(&full))
    ));
    assert!(contain_admission_with_lookup(
        &restored,
        &passenger,
        &peer_owner,
        false,
        &|_| Some(Arc::clone(&full))
    ));
    child.write().unwrap().contained_by_id = 7319;
    assert!(!contain_admission_with_lookup(
        &module,
        &passenger,
        &owner,
        true,
        &|_| Some(Arc::clone(&child))
    ));
}
