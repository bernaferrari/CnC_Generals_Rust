//! Receiving application/map OCL lifetime; definitions are actual parser products.
use super::*;
fn text(model: &str) -> String {
    format!(
        "ObjectCreationList SameOCL\n CreateDebris\n ModelNames={model}\n Count=1\n End\n End\n"
    )
}
fn model(world: &GameLogic) -> String {
    let list = world.weapon_effect_definitions.ocl("SameOCL").unwrap();
    list.nuggets()[0]
        .as_any()
        .downcast_ref::<gamelogic::object_creation_list::nuggets::GenericObjectCreationNugget>()
        .unwrap()
        .names[0]
        .clone()
}
#[test]
fn application_map_solo_reset_are_receiving_and_failed_parse_is_transactional() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "application_map_solo_reset_are_receiving_and_failed_parse_is_transactional",
        || {
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            owner
                .admit_ocl_application_sources(&[text("Default"), text("Retail")])
                .unwrap();
            peer.admit_ocl_application_sources(&[text("Peer")]).unwrap();
            for world in [&mut owner, &mut peer] {
                world.templates.insert(
                    "SameSource".into(),
                    crate::game_logic::ThingTemplate::new("SameSource"),
                );
            }
            let a = owner
                .create_object("SameSource", crate::game_logic::Team::USA, glam::Vec3::ZERO)
                .unwrap();
            let b = peer
                .create_object("SameSource", crate::game_logic::Team::USA, glam::Vec3::ZERO)
                .unwrap();
            assert_eq!(a, b);
            assert_eq!(model(&owner), "Retail");
            let base = owner.ocl_application_definitions();
            owner
                .apply_receiving_ocl_overrides(&[text("Map"), text("Solo")])
                .unwrap();
            assert_eq!(model(&owner), "Solo");
            assert_eq!(model(&peer), "Peer");
            assert!(owner
                .apply_receiving_ocl_overrides(&[
                    text("WouldHaveReplaced"),
                    "ObjectCreationList Broken\n CreateDebris\n".into()
                ])
                .is_err());
            assert_eq!(model(&owner), "Solo");
            assert!(owner
                .admit_ocl_application_sources(&[
                    text("WouldReplaceBase"),
                    "ObjectCreationList Broken\n".into()
                ])
                .is_err());
            assert!(std::sync::Arc::ptr_eq(
                &base,
                &owner.ocl_application_definitions()
            ));
            let mut candidate = GameLogic::new();
            candidate.seed_ocl_application_definitions(base.clone());
            assert_eq!(model(&candidate), "Retail");
            assert!(std::sync::Arc::ptr_eq(
                &base,
                &candidate.ocl_application_definitions()
            ));
            candidate
                .apply_receiving_ocl_overrides(&[text("ReceivingMap")])
                .unwrap();
            assert_eq!(model(&candidate), "ReceivingMap");
            assert_eq!(model(&owner), "Solo");
            owner.reset();
            assert_eq!(model(&owner), "Retail");
            assert_eq!(model(&peer), "Peer");
            assert_eq!(model(&candidate), "ReceivingMap");
        },
    );
}
