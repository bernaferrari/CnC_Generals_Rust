//! C++ AI.cpp:613-700: filters finish before priority containment enumeration.
use super::*;
use crate::ai::{AttackPriorityInfo, PartitionFilter, search_qualifiers};
use std::cell::{Cell, RefCell};
use std::sync::MutexGuard;

struct ContainerFixture {
    mood: MoodFixture,
    container_id: u32,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
    passenger_id: u32,
}

impl ContainerFixture {
    fn new(container_x: f32) -> Self {
        let mut mood = MoodFixture::new("Yes");
        // Loaded game rules use a positive divisor; bare rule defaults are zero.
        crate::ai::the_ai().write().unwrap().update_ai_data(|data| {
            data.attack_priority_distance_modifier = 1000.0;
        });
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object QueryPriorityContainer\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = OpenContain Cargo\n ContainMax = 4\n AllowInsideKindOf = INFANTRY\n AllowEnemiesInside = Yes\n End\nEnd\nObject QueryPriorityPassenger\n KindOf = INFANTRY\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n TransportSlotCount = 1\nEnd\n"
        ), 2);
        let target_team = mood
            ._factory
            .get_object(mood.target_id)
            .unwrap()
            .get_base_object()
            .unwrap()
            .read()
            .unwrap()
            .get_team()
            .unwrap();
        let container_id = create(
            &mut mood._factory,
            "QueryPriorityContainer",
            Coord3D::new(container_x, 0.0, 0.0),
            Some(target_team.clone()),
            ObjectCreationFlags::NO_AI,
        );
        let container = mood
            ._factory
            .get_object(container_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        let contain = container.read().unwrap().get_contain().unwrap();
        assert_eq!(contain.lock().unwrap().get_contain_count(), 0);
        let passenger_id = create(
            &mut mood._factory,
            "QueryPriorityPassenger",
            Coord3D::new(5000.0, 0.0, 0.0),
            Some(target_team),
            ObjectCreationFlags::NO_AI,
        );
        refresh_partition();
        let candidates = crate::helpers::ThePartitionManager::get()
            .unwrap()
            .get_objects_in_range(&Coord3D::ZERO, 1000.0);
        assert!(candidates.contains(&container_id));
        assert!(!candidates.contains(&passenger_id));
        Self {
            mood,
            container_id,
            contain,
            passenger_id,
        }
    }

    fn priority(&self) -> AttackPriorityInfo {
        let mut info = AttackPriorityInfo::new();
        info.name = "QueryPriority".into();
        info.set_priority("EnterCapacityOccupant", 10);
        info.set_priority("QueryPriorityContainer", 1);
        info.set_priority("QueryPriorityPassenger", 100);
        info
    }

    fn assert_passenger_installed(&self) {
        assert_eq!(
            self.contain
                .lock()
                .unwrap()
                .get_contained_objects()
                .as_ref(),
            &[self.passenger_id]
        );
        assert_eq!(
            self.mood
                ._factory
                .get_object(self.passenger_id)
                .unwrap()
                .get_base_object()
                .unwrap()
                .read()
                .unwrap()
                .get_contained_by(),
            Some(self.container_id)
        );
    }
}

fn refresh_partition() {
    crate::system::game_logic::get_game_logic()
        .lock()
        .unwrap()
        .partition_manager_mut()
        .update()
        .unwrap();
}

fn search(
    fixture: &MoodFixture,
    qualifiers: u32,
    info: Option<&AttackPriorityInfo>,
    filter: Option<&dyn PartitionFilter>,
    range: f32,
) -> Option<u32> {
    let before = get_game_logic_random_seed_state();
    let result = crate::ai::the_ai()
        .read()
        .unwrap()
        .find_closest_enemy_for_source(
            &fixture.source.read().unwrap(),
            range,
            qualifiers,
            info,
            filter,
        )
        .unwrap();
    assert_eq!(
        get_game_logic_random_seed_state(),
        before,
        "target filters consume no RNG"
    );
    result
}

struct InsertPassenger<'a> {
    trigger_id: u32,
    contain: &'a Mutex<dyn ContainModuleInterface + 'static>,
    passenger_id: u32,
    insertions: Cell<u32>,
}

impl PartitionFilter for InsertPassenger<'_> {
    fn allow(&self, id: u32) -> bool {
        if id == self.trigger_id {
            self.contain
                .lock()
                .unwrap()
                .contain_object(self.passenger_id)
                .unwrap();
            self.insertions.set(self.insertions.get() + 1);
        }
        true
    }
    fn debug_get_name(&self) -> &str {
        "InsertActualFactoryPassenger"
    }
}

/// Acquire only after built-in filters have completed their legitimate reads.
/// Keeping this guard proves selection does not access containment afterwards.
struct HoldContainAfterFilter<'a> {
    target_id: u32,
    contain: &'a Mutex<dyn ContainModuleInterface + 'static>,
    guard: RefCell<Option<MutexGuard<'a, dyn ContainModuleInterface + 'static>>>,
    calls: Cell<u32>,
    accept: bool,
}

impl PartitionFilter for HoldContainAfterFilter<'_> {
    fn allow(&self, id: u32) -> bool {
        if id != self.target_id {
            return false;
        }
        self.guard.replace(Some(self.contain.lock().unwrap()));
        self.calls.set(self.calls.get() + 1);
        self.accept
    }
    fn debug_get_name(&self) -> &str {
        "HoldActualContainAfterBuiltInFilters"
    }
}

#[test]
fn named_priority_reads_passengers_added_by_optional_filter() {
    if !child(concat!(
        module_path!(),
        "::named_priority_reads_passengers_added_by_optional_filter"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = ContainerFixture::new(50.0);
    let info = fixture.priority();
    crate::ai::the_ai()
        .write()
        .unwrap()
        .update_ai_data(|data| data.attack_priority_distance_modifier = 10.0);
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            None,
            1000.0
        ),
        Some(fixture.mood.target_id)
    );
    let filter = InsertPassenger {
        trigger_id: fixture.container_id,
        contain: &fixture.contain,
        passenger_id: fixture.passenger_id,
        insertions: Cell::new(0),
    };
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            Some(&filter),
            1000.0
        ),
        Some(fixture.container_id)
    );
    assert_eq!(filter.insertions.get(), 1);
    fixture.assert_passenger_installed();
}

#[test]
fn all_optional_filters_finish_before_priority_scan() {
    if !child(concat!(
        module_path!(),
        "::all_optional_filters_finish_before_priority_scan"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = ContainerFixture::new(20.0);
    let info = fixture.priority();
    // The later candidate inserts into an earlier accepted candidate.
    assert!(fixture.mood.target_id < fixture.container_id);
    // Use a later, real admitted target rather than overriding candidate order.
    let team = fixture
        .mood
        ._factory
        .get_object(fixture.mood.target_id)
        .unwrap()
        .get_base_object()
        .unwrap()
        .read()
        .unwrap()
        .get_team()
        .unwrap();
    let later_id = create(
        &mut fixture.mood._factory,
        "EnterCapacityOccupant",
        Coord3D::new(40.0, 0.0, 0.0),
        Some(team),
        ObjectCreationFlags::NO_AI,
    );
    refresh_partition();
    let filter = InsertPassenger {
        trigger_id: later_id,
        contain: &fixture.contain,
        passenger_id: fixture.passenger_id,
        insertions: Cell::new(0),
    };
    assert!(fixture.container_id < later_id);
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            Some(&filter),
            1000.0
        ),
        Some(fixture.container_id)
    );
    assert_eq!(filter.insertions.get(), 1);
    fixture.assert_passenger_installed();
}

#[test]
fn null_and_default_priority_avoid_post_filter_containment_access() {
    if !child(concat!(
        module_path!(),
        "::null_and_default_priority_avoid_post_filter_containment_access"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = ContainerFixture::new(20.0);
    let mut default_info = AttackPriorityInfo::new();
    default_info.default_priority = 0; // Default row still uses the closest path.
    for info in [None, Some(&default_info)] {
        let filter = HoldContainAfterFilter {
            target_id: fixture.container_id,
            contain: &fixture.contain,
            guard: RefCell::new(None),
            calls: Cell::new(0),
            accept: true,
        };
        assert_eq!(
            search(
                &fixture.mood,
                search_qualifiers::ATTACK_BUILDINGS,
                info,
                Some(&filter),
                1000.0
            ),
            Some(fixture.container_id)
        );
        assert_eq!(filter.calls.get(), 1);
        assert!(filter.guard.borrow().is_some());
    }
}

#[test]
fn rejected_optional_filter_avoids_priority_containment_access() {
    if !child(concat!(
        module_path!(),
        "::rejected_optional_filter_avoids_priority_containment_access"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = ContainerFixture::new(20.0);
    let info = fixture.priority();
    let filter = HoldContainAfterFilter {
        target_id: fixture.container_id,
        contain: &fixture.contain,
        guard: RefCell::new(None),
        calls: Cell::new(0),
        accept: false,
    };
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            Some(&filter),
            1000.0
        ),
        None
    );
    assert_eq!(filter.calls.get(), 1);
    assert!(filter.guard.borrow().is_some());
}

#[test]
fn zero_named_priority_avoids_post_filter_containment_access() {
    if !child(concat!(
        module_path!(),
        "::zero_named_priority_avoids_post_filter_containment_access"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = ContainerFixture::new(20.0);
    let mut info = fixture.priority();
    info.set_priority("QueryPriorityContainer", 0);
    let filter = HoldContainAfterFilter {
        target_id: fixture.container_id,
        contain: &fixture.contain,
        guard: RefCell::new(None),
        calls: Cell::new(0),
        accept: true,
    };
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            Some(&filter),
            1000.0
        ),
        None
    );
    assert_eq!(filter.calls.get(), 1);
    assert!(filter.guard.borrow().is_some());
}

#[test]
fn weapon_range_filter_is_independent_of_attack_eligibility() {
    if !child(concat!(
        module_path!(),
        "::weapon_range_filter_is_independent_of_attack_eligibility"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes");
    fixture
        ._factory
        .get_object(fixture.target_id)
        .unwrap()
        .get_base_object()
        .unwrap()
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::NoAttackFromAi.into(), true);
    assert_eq!(
        search(&fixture, search_qualifiers::CAN_ATTACK, None, None, 1000.0),
        None
    );
    assert_eq!(
        search(
            &fixture,
            search_qualifiers::WITHIN_ATTACK_RANGE,
            None,
            None,
            1000.0
        ),
        Some(fixture.target_id)
    );
    assert_eq!(
        search(
            &fixture,
            search_qualifiers::WITHIN_ATTACK_RANGE | search_qualifiers::CAN_ATTACK,
            None,
            None,
            1000.0
        ),
        None
    );
}

#[test]
fn possible_after_moving_passes_attack_filter_but_not_range_filter() {
    if !child(concat!(
        module_path!(),
        "::possible_after_moving_passes_attack_filter_but_not_range_filter"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes");
    let target = fixture
        ._factory
        .get_object(fixture.target_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    target
        .write()
        .unwrap()
        .set_position(&Coord3D::new(2000.0, 0.0, 0.0))
        .unwrap();
    refresh_partition();
    assert_eq!(
        fixture
            .source
            .read()
            .unwrap()
            .get_able_to_attack_specific_object_for_objects(
                AbleToAttackType::NewTarget,
                &target.read().unwrap(),
                CommandSourceType::FromAi
            ),
        CanAttackResult::PossibleAfterMoving
    );
    assert_eq!(
        search(&fixture, search_qualifiers::CAN_ATTACK, None, None, 3000.0),
        Some(fixture.target_id)
    );
    assert_eq!(
        search(
            &fixture,
            search_qualifiers::WITHIN_ATTACK_RANGE,
            None,
            None,
            3000.0
        ),
        None
    );
}

#[test]
fn equal_priority_uses_near_to_far_iterator_order() {
    if !child(concat!(
        module_path!(),
        "::equal_priority_uses_near_to_far_iterator_order"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = ContainerFixture::new(50.0);
    let mut info = fixture.priority();
    info.set_priority("QueryPriorityContainer", 10);
    crate::ai::the_ai().write().unwrap().update_ai_data(|data| {
        data.attack_priority_distance_modifier = 1000.0;
    });
    // The farther target has a smaller X cell and is traversed first.
    let far_target = fixture
        .mood
        ._factory
        .get_object(fixture.mood.target_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    far_target
        .write()
        .unwrap()
        .set_position(&Coord3D::new(0.0, 80.0, 0.0))
        .unwrap();
    assert!(!far_target.read().unwrap().is_off_map());
    assert!(!far_target.read().unwrap().is_effectively_dead());
    assert_eq!(
        info.get_priority(
            far_target
                .read()
                .unwrap()
                .get_template()
                .get_name()
                .as_str()
        ),
        10
    );
    refresh_partition();
    let candidates = crate::helpers::ThePartitionManager::get()
        .unwrap()
        .get_objects_in_range(&Coord3D::ZERO, 1000.0);
    assert!(
        candidates
            .iter()
            .position(|id| *id == fixture.mood.target_id)
            .unwrap()
            < candidates
                .iter()
                .position(|id| *id == fixture.container_id)
                .unwrap()
    );
    let source = fixture.mood.source.read().unwrap();
    let container = fixture
        .mood
        ._factory
        .get_object(fixture.container_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(
        crate::helpers::ThePartitionManager::get_distance_squared(
            &source,
            &far_target.read().unwrap(),
            crate::common::FROM_BOUNDING_SPHERE_2D
        ) > crate::helpers::ThePartitionManager::get_distance_squared(
            &source,
            &container.read().unwrap(),
            crate::common::FROM_BOUNDING_SPHERE_2D
        )
    );
    drop(source);
    assert!(fixture.mood.target_id < fixture.container_id);
    assert_eq!(
        search(
            &fixture.mood,
            search_qualifiers::ATTACK_BUILDINGS,
            Some(&info),
            None,
            1000.0
        ),
        Some(fixture.container_id)
    );
}
