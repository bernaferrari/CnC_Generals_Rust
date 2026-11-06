//! C++ OpenContain.cpp:856-904 admission through the authored factory interface.
//! OpenContain ignores checkCapacity; TransportContain separately enforces slots.

use crate::common::{Coord3D, DisabledType, INVALID_ID};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

#[test]
fn factory_open_admission_preserves_cpp_capacity_and_query_side_effects() {
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::factory_open_admission_preserves_cpp_capacity_and_query_side_effects"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_OPEN_ADMISSION_CHILD",
        ),
        crate::test_process::TestProcess::Child
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object AdmissionOpen\n KindOf = VEHICLE\n Behavior = OpenContain Cargo\n ContainMax = 2\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 0\n End\nEnd\nObject OpenInfantry\n KindOf = INFANTRY\nEnd\nObject OpenWrongKind\n KindOf = VEHICLE\nEnd\n"
        ),
        3,
    );
    {
        let __player_list_arc = ThePlayerList();
        let mut players = __player_list_arc.write().unwrap();
        players.clear();
        players.add_player(Arc::new(RwLock::new(Player::new(0))));
    }
    let team = Arc::new(RwLock::new(Team::new("OpenAdmissionOwn".into(), 9201)));
    team.write().unwrap().set_controlling_player_id(Some(0));
    let mut factory = ObjectFactory::new();
    let ids = {
        let mut create = |name| {
            factory
                .create_object(
                    name,
                    Coord3D::new(100.0, 110.0, 0.0),
                    Some(Arc::clone(&team)),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        [
            create("AdmissionOpen"),
            create("OpenInfantry"),
            create("OpenInfantry"),
            create("OpenInfantry"),
            create("OpenWrongKind"),
        ]
    };
    let owner = factory
        .get_object(ids[0])
        .unwrap()
        .get_base_object()
        .unwrap();
    let queried_rider = factory
        .get_object(ids[3])
        .unwrap()
        .get_base_object()
        .unwrap();
    let (contain, drawable) = {
        let owner = owner.read().unwrap();
        (
            owner.get_contain().expect("actual authored OpenContain"),
            owner.get_drawable().expect("factory drawable"),
        )
    };
    contain
        .lock()
        .unwrap()
        .set_rally_point(Coord3D::new(25.0, 35.0, 0.0));
    assert_eq!(contain.lock().unwrap().get_max_capacity(), 2);
    let save_contain = || {
        let mut bytes = Vec::new();
        contain
            .lock()
            .unwrap()
            .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
        assert!(
            !bytes.is_empty(),
            "actual Open snapshot delegates must be reachable"
        );
        bytes
    };

    // C++ deliberately ignores checkCapacity here. Require true at empty,
    // one occupant, and ContainMax occupants without admitting the query rider.
    for occupants in 0..=2 {
        let snapshot_before = save_contain();
        let flags_before = drawable.read().unwrap().get_model_conditions();
        let transport_before = owner.read().unwrap().is_transporting();
        let rider_before = {
            let rider = queried_rider.read().unwrap();
            (
                *rider.get_position(),
                rider.get_contained_by(),
                rider.is_disabled_by_type(DisabledType::Held),
            )
        };
        assert_eq!(contain.lock().unwrap().get_contained_count(), occupants);
        assert!(
            contain.lock().unwrap().can_contain(ids[3]),
            "C++ Open accepts allowed infantry with {occupants} occupants"
        );
        assert!(
            !contain.lock().unwrap().can_contain(ids[4]),
            "base kind restriction"
        );
        assert!(
            !contain.lock().unwrap().can_contain(INVALID_ID),
            "missing object"
        );
        assert_eq!(
            save_contain(),
            snapshot_before,
            "query preserves every serialized runtime field"
        );
        assert_eq!(contain.lock().unwrap().get_contained_count(), occupants);
        assert_eq!(
            drawable.read().unwrap().get_model_conditions(),
            flags_before
        );
        assert_eq!(owner.read().unwrap().is_transporting(), transport_before);
        let rider = queried_rider.read().unwrap();
        assert_eq!(
            (
                *rider.get_position(),
                rider.get_contained_by(),
                rider.is_disabled_by_type(DisabledType::Held),
            ),
            rider_before,
        );
        drop(rider);
        if occupants < 2 {
            contain
                .lock()
                .unwrap()
                .contain_object(ids[occupants + 1])
                .unwrap();
        }
    }
}

/// C++ OpenContain.cpp:58 (all-bits default allow mask), KindOf.h:163
/// (`isAnyKindOf` rejects a kindless entrant) and Object.cpp:1548 (no ObjectID
/// shortcut in `getRelationship`: an object is its own ally through its team).
#[test]
fn factory_open_default_allow_mask_kindless_entrant_and_self_relationship() {
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::factory_open_default_allow_mask_kindless_entrant_and_self_relationship"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_OPEN_DEFAULT_MASK_CHILD",
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
                    "Object DefaultMaskOpen\n KindOf = STRUCTURE\n Behavior = OpenContain Cargo\n ContainMax = 4\n NumberOfExitPaths = 0\n End\nEnd\nObject EmptyMaskOpen\n KindOf = STRUCTURE\n Behavior = OpenContain Cargo\n ContainMax = 4\n AllowInsideKindOf = NONE\n NumberOfExitPaths = 0\n End\nEnd\nObject MaskVehicle\n KindOf = VEHICLE\nEnd\nObject MaskKindless\nEnd\n"
                ),
                4,
            );
            {
                let list = ThePlayerList();
                let mut players = list.write().unwrap();
                players.clear();
                players.add_player(Arc::new(RwLock::new(Player::new(0))));
            }
            let team = Arc::new(RwLock::new(Team::new("DefaultMaskOwn".into(), 9401)));
            team.write().unwrap().set_controlling_player_id(Some(0));
            let mut factory = ObjectFactory::new();
            let mut create = |name: &str| {
                factory
                    .create_object(
                        name,
                        Coord3D::new(10.0, 20.0, 0.0),
                        Some(Arc::clone(&team)),
                        ObjectCreationFlags::NO_AI,
                    )
                    .unwrap()
            };
            let default_id = create("DefaultMaskOpen");
            let empty_id = create("EmptyMaskOpen");
            let vehicle_id = create("MaskVehicle");
            let kindless_id = create("MaskKindless");
            let contain_of = |id| {
                factory
                    .get_object(id)
                    .unwrap()
                    .get_base_object()
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_contain()
                    .expect("authored OpenContain")
            };
            let default_contain = contain_of(default_id);
            assert!(
                default_contain.lock().unwrap().can_contain(vehicle_id),
                "default allow mask admits every kind"
            );
            assert!(
                !default_contain.lock().unwrap().can_contain(kindless_id),
                "a kindless entrant matches no kind"
            );
            assert!(
                !contain_of(empty_id).lock().unwrap().can_contain(vehicle_id),
                "explicit empty allow mask admits nothing"
            );
            let owner = factory.get_object(default_id).unwrap().get_base_object().unwrap();
            let owner = owner.read().unwrap();
            assert_eq!(
                owner.get_relationship_to(&owner),
                crate::object::contain::open_contain::ObjectRelationship::Ally
            );
        })
    });
}
