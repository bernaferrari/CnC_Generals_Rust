use game_engine::system::geometry::{GeometryInfo as Shape, GeometryType};
use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType};
use gamelogic::ai::pathfind_complete::{PathfindingSystem, SURFACE_GROUND, UNINITIALIZED_ZONE};
use gamelogic::common::{
    AsciiString, DefaultThingTemplate, GeometryInfo, KindOf, ObjectStatusMaskType, ThingTemplate,
};
use gamelogic::object::Object;
use glam::Vec3;
use std::sync::Arc;

#[derive(Debug)]
struct FenceTemplate(DefaultThingTemplate);

impl ThingTemplate for FenceTemplate {
    fn get_name(&self) -> &AsciiString {
        self.0.get_name()
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.0.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.0.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.0.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.0.is_kind_of(kind)
    }
    fn get_fence_width(&self) -> f32 {
        40.0
    }
    fn get_fence_x_offset(&self) -> f32 {
        20.0
    }
}

#[test]
fn two_unregistered_structures_with_the_same_id_use_their_own_footprints() {
    let mut template = DefaultThingTemplate::new("FootprintStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    template.add_kind_of(KindOf::CanSeeThrough);
    let mut opaque = DefaultThingTemplate::new("OpaqueStructure".into());
    opaque.add_kind_of(KindOf::Structure);
    opaque.add_kind_of(KindOf::Immobile);
    let mut first = Object::new_raw(Arc::new(template), 41, ObjectStatusMaskType::NONE, None);
    let mut second = Object::new_raw(Arc::new(opaque), 41, ObjectStatusMaskType::NONE, None);
    for (object, x) in [(&mut first, 55.0), (&mut second, 25.0)] {
        let shape = Shape::new(GeometryType::Box, false, 10.0, 5.0, 10.0);
        object.set_geometry_info(GeometryInfo {
            shape,
            ..GeometryInfo::default()
        });
        object.set_position(&Vec3::new(x, 55.0, 0.0)).unwrap();
    }

    let mut first_world = PathfindingSystem::new(10, 10);
    let mut second_world = PathfindingSystem::new(10, 10);
    first_world.classify_object_footprint(&first).unwrap();
    second_world.classify_object_footprint(&second).unwrap();
    let expected = [(4, 5), (5, 5), (6, 5), (4, 6), (5, 6), (6, 6)];
    for x in 0..10 {
        for y in 0..10 {
            let cell = GridCoord::new(x, y);
            let occupied = expected.contains(&(x, y));
            let expected_type = if occupied {
                PathfindCellType::Obstacle
            } else {
                PathfindCellType::Clear
            };
            assert_eq!(
                first_world.get_cell_type(&cell.to_world(0.0)),
                Some(expected_type),
                "cell {x},{y}"
            );
            assert_eq!(
                first_world.get_cell_obstacle_id(cell),
                occupied.then_some(41)
            );
        }
    }
    let first_only = GridCoord::new(6, 5).to_world(0.0);
    assert!(first_world.is_cell_obstacle_transparent(GridCoord::new(6, 5)));
    assert!(!second_world.is_cell_obstacle_transparent(GridCoord::new(1, 5)));
    assert_eq!(
        second_world.get_cell_type(&first_only),
        Some(PathfindCellType::Clear)
    );
    first_world
        .classify_object_footprint_ex(&first, false)
        .unwrap();
    assert_eq!(
        first_world.get_cell_type(&first_only),
        Some(PathfindCellType::Clear)
    );
    let second_only = GridCoord::new(1, 5).to_world(0.0);
    assert_eq!(
        second_world.get_cell_type(&second_only),
        Some(PathfindCellType::Obstacle)
    );
}

#[test]
fn zero_width_box_does_not_invent_a_footprint() {
    let mut template = DefaultThingTemplate::new("EmptyStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_raw(Arc::new(template), 43, ObjectStatusMaskType::NONE, None);
    object.set_geometry_info(GeometryInfo {
        shape: Shape::new(GeometryType::Box, false, 0.0, 5.0, 10.0),
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    let mut world = PathfindingSystem::new(10, 10);
    world.set_zone_bridge(GridCoord::new(5, 5), true);
    world.classify_object_footprint(&object).unwrap();
    assert!(!world.zone_interacts_with_bridge(GridCoord::new(5, 5)));
    assert_eq!(
        world.get_cell_type(&Vec3::new(55.0, 55.0, 0.0)),
        Some(PathfindCellType::Clear)
    );
}

#[test]
fn fence_classification_preserves_the_crusher_exception_without_registry_access() {
    let template = FenceTemplate(DefaultThingTemplate::new("Fence".into()));
    let mut object = Object::new_raw(Arc::new(template), 44, ObjectStatusMaskType::NONE, None);
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    let mut world = PathfindingSystem::new(10, 10);
    world.classify_object_footprint(&object).unwrap();
    for x in 3..=6 {
        let pos = GridCoord::new(x, 5).to_world(0.0);
        assert_eq!(world.get_cell_obstacle_id(GridCoord::new(x, 5)), Some(44));
        assert!(!world.valid_movement_position(SURFACE_GROUND, false, &pos, None));
        assert!(world.valid_movement_position(SURFACE_GROUND, true, &pos, None));
    }
    let cell = GridCoord::new(5, 5);
    world.set_zone_bridge(cell, true);
    world.classify_object_footprint(&object).unwrap();
    assert!(
        world.zone_interacts_with_bridge(cell),
        "an unchanged fence does not repair zones"
    );
    world.classify_object_footprint_ex(&object, false).unwrap();
    assert!(!world.zone_interacts_with_bridge(cell));
}

#[test]
fn spherical_structure_matches_the_original_cell_raster() {
    let mut template = DefaultThingTemplate::new("RoundStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_raw(Arc::new(template), 42, ObjectStatusMaskType::NONE, None);
    let shape = Shape::new(GeometryType::Sphere, false, 10.0, 10.0, 10.0);
    object.set_geometry_info(GeometryInfo {
        shape,
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    let mut world = PathfindingSystem::new(10, 10);
    world.classify_object_footprint(&object).unwrap();
    let expected = [(4, 5), (5, 4), (5, 5), (5, 6), (6, 5)];
    for x in 0..10 {
        for y in 0..10 {
            let pos = GridCoord::new(x, y).to_world(0.0);
            let cell_type = if expected.contains(&(x, y)) {
                PathfindCellType::Obstacle
            } else {
                PathfindCellType::Clear
            };
            assert_eq!(world.get_cell_type(&pos), Some(cell_type), "cell {x},{y}");
        }
    }
}

#[cfg(feature = "internal")]
#[test]
fn body_observation_failure_is_returned_before_mutating_the_grid() {
    let mut template = DefaultThingTemplate::new("PoisonedStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_test_from_template(45, 100.0, Arc::new(template));
    object.set_geometry_info(GeometryInfo {
        shape: Shape::new(GeometryType::Box, false, 10.0, 5.0, 10.0),
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    let body = object.get_body_module().unwrap();
    let _ = std::thread::spawn(move || {
        let _guard = body.lock().unwrap();
        panic!("inject body lock failure");
    })
    .join();
    let mut world = PathfindingSystem::new(10, 10);
    assert!(world.classify_object_footprint(&object).is_err());
    assert_eq!(
        world.get_cell_type(&Vec3::new(55.0, 55.0, 0.0)),
        Some(PathfindCellType::Clear)
    );
}

#[cfg(feature = "internal")]
#[test]
fn rubble_body_produces_an_ownerless_rubble_footprint() {
    let mut template = DefaultThingTemplate::new("RubbleStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_test_from_template(46, 100.0, Arc::new(template));
    object.set_geometry_info(GeometryInfo {
        shape: Shape::new(GeometryType::Box, false, 10.0, 5.0, 10.0),
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    {
        let body = object.get_body_module().unwrap();
        let mut body = body.lock().unwrap();
        body.set_damage_state(gamelogic::common::BodyDamageType::Rubble)
            .unwrap();
        assert_eq!(
            body.get_damage_state(),
            gamelogic::common::BodyDamageType::Rubble
        );
    }
    let mut world = PathfindingSystem::new(10, 10);
    world.classify_object_footprint(&object).unwrap();
    let cell = GridCoord::new(5, 5);
    assert_eq!(
        world.get_cell_type(&cell.to_world(0.0)),
        Some(PathfindCellType::Rubble)
    );
    assert_eq!(world.get_cell_obstacle_id(cell), None);
    world.classify_object_footprint_ex(&object, false).unwrap();
    assert_eq!(
        world.get_cell_type(&cell.to_world(0.0)),
        Some(PathfindCellType::Clear)
    );
}

#[cfg(feature = "internal")]
#[test]
fn rubble_fence_removal_keeps_the_original_did_anything_gate() {
    let template = FenceTemplate(DefaultThingTemplate::new("RubbleFence".into()));
    let mut object = Object::new_test_from_template(47, 100.0, Arc::new(template));
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    object
        .get_body_module()
        .unwrap()
        .lock()
        .unwrap()
        .set_damage_state(gamelogic::common::BodyDamageType::Rubble)
        .unwrap();
    let mut world = PathfindingSystem::new(10, 10);
    let cell = GridCoord::new(5, 5);
    world.classify_object_footprint(&object).unwrap();
    assert_eq!(
        world.get_cell_type(&cell.to_world(0.0)),
        Some(PathfindCellType::Rubble)
    );
    world.set_zone_bridge(cell, true);
    world.classify_object_footprint_ex(&object, false).unwrap();
    assert_eq!(
        world.get_cell_type(&cell.to_world(0.0)),
        Some(PathfindCellType::Clear)
    );
    assert!(world.zone_interacts_with_bridge(cell));
}

#[test]
fn uninitialized_zone_matches_the_original_value() {
    assert_eq!(UNINITIALIZED_ZONE, 0);
}

#[test]
fn footprint_update_clears_only_its_worlds_intersecting_bridge_block() {
    let mut template = DefaultThingTemplate::new("ZoneStructure".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_raw(Arc::new(template), 50, ObjectStatusMaskType::NONE, None);
    object.set_geometry_info(GeometryInfo {
        shape: Shape::new(GeometryType::Box, false, 10.0, 5.0, 10.0),
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(55.0, 55.0, 0.0)).unwrap();
    let mut world = PathfindingSystem::new(30, 20);
    let mut peer = PathfindingSystem::new(30, 20);
    world.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();
    let touched = GridCoord::new(5, 5);
    let untouched = GridCoord::new(25, 5);
    for pathfinder in [&mut world, &mut peer] {
        pathfinder.set_zone_bridge(touched, true);
        pathfinder.set_zone_bridge(untouched, true);
    }
    world.classify_object_footprint(&object).unwrap();
    assert!(!world.zone_interacts_with_bridge(touched));
    assert!(world.zone_interacts_with_bridge(untouched));
    assert!(peer.zone_interacts_with_bridge(touched));
    world.set_zone_bridge(touched, true);
    world.classify_object_footprint(&object).unwrap();
    assert!(
        !world.zone_interacts_with_bridge(touched),
        "box zone repair runs even when cells are unchanged"
    );
    world.set_zone_bridge(touched, true);
    world.classify_object_footprint_ex(&object, false).unwrap();
    assert!(!world.zone_interacts_with_bridge(touched));
    assert!(peer.zone_interacts_with_bridge(touched));
}
