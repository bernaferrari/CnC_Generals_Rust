//! hq-yqalp: derived ContainModuleInterface::can_contain must propagate
//! OBJECT_REGISTRY.with_object predicate results (C++ Overlord.cpp:412,
//! Heal inherits OpenContain admission). Missing id → false.

use crate::common::{Coord3D, INVALID_ID};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, RwLock};

#[test]
fn derived_overlord_and_heal_can_contain_propagate_registry_predicate() {
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::derived_overlord_and_heal_can_contain_propagate_registry_predicate"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_DERIVED_CONTAIN_ADMISSION_CHILD",
        ),
        crate::test_process::TestProcess::Child
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    let services = crate::system::engine_stores::new_world_services();
    game_engine::common::common_services::with_common_services(&common, || {
        crate::system::engine_stores::with_world_services(&services, || {
            if get_thing_factory().unwrap().is_none() {
                init_thing_factory().unwrap();
            }
            if get_module_factory().unwrap().is_none() {
                init_module_factory().unwrap();
            }
            crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
            assert_eq!(
                get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
                    "Object YqalpOverlord\n KindOf = VEHICLE\n Behavior = OverlordContain Cargo\nSlots = 3\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 0\n End\nEnd\nObject YqalpHealPad\n KindOf = STRUCTURE\n Behavior = HealContain Cargo\n ContainMax = 2\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 0\n End\nEnd\nObject YqalpInfantry\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject YqalpWrongKind\n KindOf = VEHICLE\n TransportSlotCount = 1\nEnd\n"
                ),
                4,
            );
            {
                let __player_list_arc = ThePlayerList();
                let mut players = __player_list_arc.write().unwrap();
                players.clear();
                players.add_player(Arc::new(RwLock::new(Player::new(0))));
            }
            let team = Arc::new(RwLock::new(Team::new("YqalpOwn".into(), 9301)));
            team.write().unwrap().set_controlling_player_id(Some(0));
            let mut factory = ObjectFactory::new();
            let mut create = |name: &str| {
                factory
                    .create_object(
                        name,
                        Coord3D::new(40.0, 50.0, 0.0),
                        Some(Arc::clone(&team)),
                        ObjectCreationFlags::NO_AI,
                    )
                    .unwrap()
            };
            let overlord_id = create("YqalpOverlord");
            let heal_id = create("YqalpHealPad");
            let infantry_id = create("YqalpInfantry");
            let wrong_id = create("YqalpWrongKind");

            let overlord_contain = {
                let obj = factory
                    .get_object(overlord_id)
                    .unwrap()
                    .get_base_object()
                    .unwrap();
                obj.read()
                    .unwrap()
                    .get_contain()
                    .expect("authored OverlordContain")
            };
            assert!(
                overlord_contain.lock().unwrap().can_contain(infantry_id),
                "hq-yqalp: Overlord can_contain must not discard true registry predicate"
            );
            assert!(!overlord_contain.lock().unwrap().can_contain(INVALID_ID));
            assert!(
                !overlord_contain.lock().unwrap().can_contain(wrong_id),
                "kind restriction still applies"
            );

            let heal_contain = {
                let obj = factory
                    .get_object(heal_id)
                    .unwrap()
                    .get_base_object()
                    .unwrap();
                obj.read()
                    .unwrap()
                    .get_contain()
                    .expect("authored HealContain")
            };
            assert!(
                heal_contain.lock().unwrap().can_contain(infantry_id),
                "hq-yqalp: Heal can_contain must not discard true registry predicate"
            );
            assert!(!heal_contain.lock().unwrap().can_contain(INVALID_ID));
            assert!(!heal_contain.lock().unwrap().can_contain(wrong_id));
        })
    });
}
