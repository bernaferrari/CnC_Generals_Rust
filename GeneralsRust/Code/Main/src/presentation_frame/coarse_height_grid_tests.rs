use super::*;
use gamelogic::common::{AsciiString, BodyDamageType, Coord3D};
use gamelogic::path::PathfindLayerEnum;
use gamelogic::terrain::{BridgeInfo, TerrainLogic};

fn publish(logic: &mut GameLogic) -> PresentationFrame {
    PresentationFrame::try_publish_for_engine_with_runtime_heightmap(logic, 0, None, None, false)
        .expect("receiving presentation publication")
}

fn add_bridge(terrain: &mut TerrainLogic, id: u32, height: f32) {
    let mut info = BridgeInfo::new();
    info.bridge_object_id = id;
    info.from_left = Coord3D::new(-10.0, -10.0, height);
    info.from_right = Coord3D::new(-10.0, 10.0, height);
    info.to_left = Coord3D::new(10.0, -10.0, height);
    info.to_right = Coord3D::new(10.0, 10.0, height);
    terrain.prepend_bridge_on_layer(
        info,
        AsciiString::from("ProjectionBridge"),
        PathfindLayerEnum::Bridge1,
    );
}

fn assert_scalar_and_wire(logic: &GameLogic, env: &PresentationWorldEnv) {
    let (min, max) = logic.world_bounds();
    let span = (max - min).max(glam::Vec3::ONE);
    let heights: Vec<_> = (0..4096)
        .map(|i| {
            let u = ((i % 64) as f32 + 0.5) / 64.0;
            let v = ((i / 64) as f32 + 0.5) / 64.0;
            logic.terrain_height_at(glam::Vec3::new(min.x + u * span.x, 0.0, min.z + v * span.z))
        })
        .collect();
    assert_eq!(
        env.height_samples_from_terrain,
        heights.iter().any(Option::is_some)
    );
    let expected = heights
        .into_iter()
        .map(|height| height.unwrap_or(0.0))
        .collect::<Vec<_>>();
    assert!(
        env.height_samples
            .iter()
            .zip(&expected)
            .all(|(got, expected)| got.to_bits() == expected.to_bits())
    );
    assert_eq!(env.height_samples.len(), expected.len());
    assert_eq!(
        serde_json::to_vec(&env.height_samples).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
}

#[test]
fn receiving_publication_shares_unchanged_grid_and_observes_bridge_order_resize_and_fallback() {
    let mut world = GameLogic::new();
    world.override_world_size(64.0, 64.0);
    let terrain = Arc::clone(world.world_services.terrain());
    {
        let mut terrain = terrain.write().unwrap();
        terrain
            .try_restore_logic_height_map_with_effects(8, 8, &[0; 64], &mut || {})
            .unwrap();
        add_bridge(&mut terrain, 100, 20.);
        add_bridge(&mut terrain, 200, 40.);
    }
    let first = publish(&mut world);
    let repeated = publish(&mut world);
    assert_eq!(first.frame, repeated.frame);
    assert_scalar_and_wire(&world, &first.world_env);
    assert_eq!(first.world_env.sample_height(0.0, 0.0), Some(40.));
    assert_eq!(
        first.world_env.height_samples.as_ptr(),
        repeated.world_env.height_samples.as_ptr(),
        "two native publications rebuilt the same receiving terrain grid"
    );

    let revision = terrain.read().unwrap().height_revision();
    {
        let mut terrain = terrain.write().unwrap();
        let low = terrain.detach_bridge_by_object_id(100).unwrap();
        terrain.prepend_bridge_on_layer(
            low.get_bridge_info().clone(),
            AsciiString::from("ProjectionBridge"),
            PathfindLayerEnum::Bridge1,
        );
    }
    let reordered = publish(&mut world).world_env;
    assert_eq!(terrain.read().unwrap().height_revision(), revision);
    assert_eq!(reordered.sample_height(0.0, 0.0), Some(20.));
    assert_ne!(
        reordered.height_samples.as_ptr(),
        first.world_env.height_samples.as_ptr()
    );
    assert_scalar_and_wire(&world, &reordered);

    assert!(
        terrain
            .write()
            .unwrap()
            .observe_host_bridge_body(100, Some(BodyDamageType::Rubble))
            .is_some()
    );
    let broken = publish(&mut world).world_env;
    assert_eq!(terrain.read().unwrap().height_revision(), revision);
    assert_eq!(broken.sample_height(0.0, 0.0), Some(0.));
    assert_scalar_and_wire(&world, &broken);

    assert!(
        terrain
            .write()
            .unwrap()
            .observe_host_bridge_body(100, Some(BodyDamageType::Pristine))
            .is_some()
    );
    let repaired = publish(&mut world).world_env;
    assert_eq!(repaired.sample_height(0.0, 0.0), Some(20.));
    assert_scalar_and_wire(&world, &repaired);

    terrain
        .write()
        .unwrap()
        .try_restore_logic_height_map_with_effects(8, 8, &[2; 64], &mut || {})
        .unwrap();
    let changed_ground = publish(&mut world).world_env;
    assert_ne!(
        changed_ground.height_samples.as_ptr(),
        repaired.height_samples.as_ptr()
    );
    assert!(
        changed_ground
            .sample_height(22.0, 25.0)
            .is_some_and(|height| height > 0.0)
    );
    assert_scalar_and_wire(&world, &changed_ground);

    world.override_world_size(128.0, 96.0);
    let resized = publish(&mut world).world_env;
    assert_ne!(
        resized.height_samples.as_ptr(),
        changed_ground.height_samples.as_ptr()
    );
    assert_scalar_and_wire(&world, &resized);
    assert_eq!(first.world_env.sample_height(0.0, 0.0), Some(40.));

    terrain.write().unwrap().reset();
    let unloaded = publish(&mut world).world_env;
    assert!(!unloaded.height_samples_from_terrain);
    assert_scalar_and_wire(&world, &unloaded);
    assert!(world.restore_terrain_heights_from_grid(2, 2, &[7.0; 4]));
    let fallback = publish(&mut world).world_env;
    assert_eq!(fallback.sample_height(0.0, 0.0), Some(7.0));
    assert_scalar_and_wire(&world, &fallback);
}
