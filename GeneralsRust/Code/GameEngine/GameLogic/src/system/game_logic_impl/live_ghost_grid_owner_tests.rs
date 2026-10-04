//! Live-parent ghost visibility reads the driving GameLogic partition grid.
//! CPP PartitionManager.cpp:1582-1688 preserves the exact query-time seen
//! history; Object.cpp:1778-1788 handles always-visible / no-partition owners.
//! This does not prove orphan ownership, complete ghost capture, or whole-world
//! isolation: player relationships and renderer ghost registration remain ambient.

use super::*;
use crate::common::ObjectShroudStatus;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use std::sync::{Arc, RwLock};

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_LIVE_GHOST_GRID_OWNER_CHILD",
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
    use game_engine::common::thing::thing_factory::{
        ensure_thing_factory_exists, get_thing_factory,
    };
    assert!(ensure_thing_factory_exists());
    let count = get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object LiveGhostGridBuilding\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 4\n GeometryMinorRadius = 4\n GeometryHeight = 4\n GeometryIsSmall = Yes\nEnd\n",
    );
    assert_eq!(count, 1);
}

struct WorldFixture {
    world: GameLogic,
    _factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    id: ObjectID,
}

impl WorldFixture {
    fn new() -> Self {
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "LiveGhostGridBuilding",
                Coord3D::new(20.0, 20.0, 0.0),
                None,
                ObjectCreationFlags::NO_DRAWABLE | ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
        assert!(owner.read().unwrap().is_kind_of(KindOf::Immobile));
        let mut world = GameLogic::new();
        assert_eq!(world.register_object(Arc::clone(&owner)).unwrap(), id);
        assert!(Arc::ptr_eq(world.objects.get(&id).unwrap(), &owner));
        assert!(world.partition_manager.ghost_link_scene_id(id).is_some());
        Self {
            world,
            _factory: factory,
            owner,
            id,
        }
    }

    fn query(&self) -> ObjectShroudStatus {
        self.world.ghost_shroud_status_for_link(self.id, 0)
    }
}

/// Save and restore the exact residual manager in this private child. This
/// chooses a deliberately foreign authority only to witness OLD wrong-grid
/// behavior; it is not production publication or a whole-registry reset.
struct RestorePrimary(Option<crate::object::collide::partition_manager::PartitionManager>);
impl RestorePrimary {
    fn install(updated: bool, visible: bool) -> Self {
        use crate::object::collide::partition_manager::{PARTITION_MANAGER, PartitionManager};
        let mut foreign = PartitionManager::new();
        if visible {
            foreign.do_shroud_reveal_cells(&Coord3D::new(20.0, 20.0, 0.0), 1.0, 1);
        } else if updated {
            foreign.do_shroud_cover_cells(&Coord3D::new(20.0, 20.0, 0.0), 1.0, 1);
        }
        Self(Some(std::mem::replace(
            &mut *PARTITION_MANAGER.write().unwrap(),
            foreign,
        )))
    }
}
impl Drop for RestorePrimary {
    fn drop(&mut self) {
        *crate::object::collide::partition_manager::PARTITION_MANAGER
            .write()
            .unwrap() = self.0.take().unwrap();
    }
}

#[test]
fn live_ghost_same_id_queries_use_distinct_driving_grids_and_seen_history() {
    if !child(concat!(
        module_path!(),
        "::live_ghost_same_id_queries_use_distinct_driving_grids_and_seen_history"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let mut first = WorldFixture::new();
    let mut second = WorldFixture::new();
    assert_eq!(first.id, second.id);
    assert!(!Arc::ptr_eq(&first.owner, &second.owner));
    assert_ne!(
        first.world.partition_manager.ghost_link_scene_id(first.id),
        second
            .world
            .partition_manager
            .ghost_link_scene_id(second.id)
    );
    let pos = Coord3D::new(20.0, 20.0, 0.0);
    first.world.partition_manager.do_shroud_reveal(&pos, 1.0, 1);
    second.world.partition_manager.do_shroud_cover(&pos, 1.0, 1);
    let _primary = RestorePrimary::install(true, false);
    assert_eq!(first.query(), ObjectShroudStatus::Clear);
    assert_eq!(second.query(), ObjectShroudStatus::Shrouded);
    first
        .world
        .partition_manager
        .undo_shroud_reveal(&pos, 1.0, 1);
    assert_eq!(first.query(), ObjectShroudStatus::Fogged);
    assert_eq!(second.query(), ObjectShroudStatus::Shrouded);
    second
        .world
        .partition_manager
        .do_shroud_reveal(&pos, 1.0, 1);
    assert_eq!(second.query(), ObjectShroudStatus::Clear);
    assert_eq!(first.query(), ObjectShroudStatus::Fogged);
    first.world.partition_manager.do_shroud_cover(&pos, 1.0, 1);
    assert_eq!(first.query(), ObjectShroudStatus::Shrouded);
    first
        .world
        .partition_manager
        .undo_shroud_cover(&pos, 1.0, 1);
    first.world.partition_manager.do_shroud_reveal(&pos, 1.0, 1);
    first
        .world
        .partition_manager
        .undo_shroud_reveal(&pos, 1.0, 1);
    assert_eq!(
        first.query(),
        ObjectShroudStatus::Shrouded,
        "cover must forget prior sighting"
    );
}

#[test]
fn live_ghost_query_can_borrow_grid_while_its_game_logic_mutex_is_held() {
    if !child(concat!(
        module_path!(),
        "::live_ghost_query_can_borrow_grid_while_its_game_logic_mutex_is_held"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let fixture = WorldFixture::new();
    let _primary = RestorePrimary::install(false, false);
    // Factory admitted this exact owner through the Helpers adapter, which
    // tracks the canonical update list but does not attach partition ghosts.
    // Exercise the existing full GameLogic registration path for this owner;
    // no private ghost map/link insertion and no additional object allocation.
    let mut driving = get_game_logic().lock().unwrap();
    assert!(Arc::ptr_eq(
        driving.objects.get(&fixture.id).unwrap(),
        &fixture.owner
    ));
    assert_eq!(
        driving.register_object(Arc::clone(&fixture.owner)).unwrap(),
        fixture.id
    );
    assert!(Arc::ptr_eq(
        driving.objects.get(&fixture.id).unwrap(),
        &fixture.owner
    ));
    assert!(
        driving
            .partition_manager
            .ghost_link_scene_id(fixture.id)
            .is_some()
    );
    driving
        .partition_manager
        .do_shroud_reveal(&Coord3D::new(20.0, 20.0, 0.0), 1.0, 1);
    assert_eq!(
        driving.ghost_shroud_status_for_link(fixture.id, 0),
        ObjectShroudStatus::Clear
    );
}

#[test]
fn foreign_updated_grid_cannot_make_cold_driving_grid_valid() {
    if !child(concat!(
        module_path!(),
        "::foreign_updated_grid_cannot_make_cold_driving_grid_valid"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let fixture = WorldFixture::new();
    assert!(!fixture.world.partition_manager.updated_since_last_reset());
    let _primary = RestorePrimary::install(true, true);
    assert_eq!(fixture.query(), ObjectShroudStatus::Invalid);
}
