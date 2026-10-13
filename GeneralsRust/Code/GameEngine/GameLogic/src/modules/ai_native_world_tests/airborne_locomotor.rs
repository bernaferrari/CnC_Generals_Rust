//! Object.cpp3336: actual selected legal surfaces, never AirborneTarget height.
use super::*;
use crate::object::contain::transport_contain::selected_locomotor_is_airborne;

#[test]
fn actual_selected_air_locomotor_ignores_targeting_height_and_is_world_owned() {
    let first = containment_lifecycle::lifecycle_world(0.0, false);
    let peer = containment_lifecycle::lifecycle_world(0.0, false);
    let owner = Arc::clone(&first.source);
    assert_eq!(
        owner.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    let peer_ai = peer
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    // Replace only this receiving selected member's immutable definition, as
    // locomotor-set selection does. No global store or mutable definition.
    let mut installed = false;
    ai.lock().unwrap().with_cur_locomotor_mut(&mut |locomotor| {
        let mut definition = (*locomotor.template).clone();
        definition.surfaces = crate::locomotor::SURFACE_AIR;
        locomotor.template = Arc::new(definition);
        installed = true;
    });
    assert!(installed, "real factory AI must have a selected locomotor");
    let status = crate::common::ObjectStatusMaskType::from_status(
        crate::common::ObjectStatusTypes::AirborneTarget,
    );
    owner.write().unwrap().set_status(status, false);
    assert!(!owner.read().unwrap().is_airborne_target());
    assert!(selected_locomotor_is_airborne(&*ai.lock().unwrap()));
    peer.source.write().unwrap().set_status(status, true);
    assert!(peer.source.read().unwrap().is_airborne_target());
    assert!(
        !selected_locomotor_is_airborne(&*peer_ai.lock().unwrap()),
        "height-only airborne status cannot substitute for the selected ground locomotor"
    );
    owner.write().unwrap().set_status(status, true);
    assert!(selected_locomotor_is_airborne(&*ai.lock().unwrap()));
}
