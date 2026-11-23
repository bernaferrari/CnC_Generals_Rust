use super::*;
use crate::common::DefaultThingTemplate;

#[test]
fn slope_orientation_uses_receiving_terrain_and_preserves_altitude() {
    let _lock = crate::test_sync::lock();
    fn terrain_world(sloped: bool) -> Arc<crate::system::engine_stores::EngineStores> {
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.boundaries = vec![crate::common::ICoord2D::new(10, 10)];
        map.heightmap = (0..100)
            .map(|sample| {
                if sloped {
                    ((sample % 10) * 16) as u8
                } else {
                    0
                }
            })
            .collect();
        let world = crate::system::engine_stores::new_for_world();
        world
            .services()
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
        world
    }

    let owner = terrain_world(true);
    let foreign = terrain_world(false);
    let mut template = DefaultThingTemplate::new("SlopeOrientation".to_string());
    template.add_kind_of(KindOf::StickToTerrainSlope);
    let mut object = Object::new_test_from_template(123, 100.0, Arc::new(template));
    object.bind_owner_world_for_test(owner.services());
    object
        .set_position(&Coord3D::new(30.0, 30.0, 10.0))
        .unwrap();
    object.set_height_above_terrain(7.0);

    let angle = 0.4;
    let mut expected = Matrix3D::IDENTITY;
    let position = object.get_transform_matrix().w_axis.truncate();
    owner.services().terrain().read().unwrap().align_on_terrain(
        angle,
        &position,
        true,
        &mut expected,
    );
    assert_ne!(
        expected.z_axis.truncate(),
        glam::Vec3::Z,
        "fixture must have a real slope"
    );

    crate::system::engine_stores::with_active_stores(&foreign, || {
        object
            .set_orientation(angle)
            .expect("orient on the receiving map");
        assert_eq!(
            object.get_transform_matrix(),
            expected,
            "foreign flat terrain must not supply alignment"
        );
        assert_eq!(
            object.get_height_above_terrain(),
            7.0,
            "Thing::setOrientation keeps cached altitude"
        );

        let terrain_write = owner.services().terrain().write().unwrap();
        assert!(
            object.set_orientation(angle + 0.3).is_err(),
            "a conflicting terrain borrow must be reported"
        );
        assert_eq!(
            object.get_transform_matrix(),
            expected,
            "the failed orientation cannot change pose"
        );
        assert_eq!(object.get_height_above_terrain(), 7.0);
        drop(terrain_write);

        object
            .set_orientation(angle + 0.3)
            .expect("retry after dropping the terrain borrow");
        assert_ne!(object.get_transform_matrix(), expected);
    });
}

#[test]
fn upright_orientation_keeps_height_while_terrain_is_loaned() {
    let _lock = crate::test_sync::lock();
    let world = crate::system::engine_stores::new_for_world();
    let mut object = Object::new_test(127, 100.0);
    object.bind_owner_world_for_test(world.services());
    object.set_height_above_terrain(11.0);

    crate::system::engine_stores::with_active_stores(&world, || {
        let _terrain_write = world.services().terrain().write().unwrap();
        object
            .set_orientation(0.4)
            .expect("upright turn has no terrain dependency");
        assert_eq!(object.get_orientation(), 0.4);
        assert_eq!(object.get_height_above_terrain(), 11.0);
    });
}
