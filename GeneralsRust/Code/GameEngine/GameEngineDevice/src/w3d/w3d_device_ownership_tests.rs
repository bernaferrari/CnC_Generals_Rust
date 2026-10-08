use super::*;

#[tokio::test]
async fn constructor_leaves_device_uninitialized() {
    let device = W3DDevice::new_with_config(W3DConfig::default())
        .await
        .unwrap();

    assert!(!device.get_status().await.unwrap().initialized);
    assert!(!device.get_status().await.unwrap().active);
    assert!(device.adapter.read().await.is_none());
    assert!(device.device.read().await.is_none());
    assert!(device.queue.read().await.is_none());
    assert!(device.surface.read().await.is_none());
    assert!(device.surface_config.read().await.is_none());
    assert!(device.renderer.read().await.is_none());
    assert!(device.graphics_context.read().await.is_none());
    assert!(device.meshes.read().await.is_empty());
    assert!(device.materials.read().await.is_empty());
    assert!(device.textures.read().await.is_empty());
    assert!(device.shaders.read().await.is_empty());
    assert_eq!(device.get_statistics().await.frame_time_ms, 0.0);
}

#[tokio::test]
async fn clone_shares_retained_device_state_and_shutdown() {
    let device = W3DDevice::new().await.unwrap();
    let clone = device.clone();

    assert!(Arc::ptr_eq(&device.config, &clone.config));
    assert!(Arc::ptr_eq(&device.instance, &clone.instance));
    assert!(Arc::ptr_eq(&device.adapter, &clone.adapter));
    assert!(Arc::ptr_eq(&device.device, &clone.device));
    assert!(Arc::ptr_eq(&device.queue, &clone.queue));
    assert!(Arc::ptr_eq(&device.surface, &clone.surface));
    assert!(Arc::ptr_eq(&device.surface_config, &clone.surface_config));
    assert!(Arc::ptr_eq(&device.renderer, &clone.renderer));
    assert!(Arc::ptr_eq(
        &device.graphics_context,
        &clone.graphics_context
    ));
    assert!(Arc::ptr_eq(&device.statistics, &clone.statistics));
    assert!(Arc::ptr_eq(&device.meshes, &clone.meshes));
    assert!(Arc::ptr_eq(&device.materials, &clone.materials));
    assert!(Arc::ptr_eq(&device.textures, &clone.textures));
    assert!(Arc::ptr_eq(&device.shaders, &clone.shaders));
    assert!(Arc::ptr_eq(&device.current_scene, &clone.current_scene));
    assert!(Arc::ptr_eq(&device.initialized, &clone.initialized));

    let mut scene = device.get_scene().await;
    scene.id = "shared-scene".into();
    scene.background_color = [0.2, 0.4, 0.6, 1.0];
    clone.set_scene(scene).await.unwrap();
    assert_eq!(device.get_scene().await.id, "shared-scene");
    assert_eq!(
        device.get_scene().await.background_color,
        [0.2, 0.4, 0.6, 1.0]
    );
    device.update_statistics(20.0).await;
    assert_eq!(clone.get_statistics().await.frame_time_ms, 20.0);
    assert_eq!(clone.get_statistics().await.fps, 50.0);
    clone.shutdown().await.unwrap();
    assert!(!device.get_status().await.unwrap().initialized);
    // Shutdown retains the scene and statistics, as the production API specifies.
    assert_eq!(device.get_scene().await.id, "shared-scene");
    assert_eq!(device.get_statistics().await.fps, 50.0);
}

#[tokio::test]
async fn separately_constructed_devices_keep_scene_and_statistics_isolated() {
    let first = W3DDevice::new().await.unwrap();
    let second = W3DDevice::new().await.unwrap();
    assert!(!Arc::ptr_eq(&first.current_scene, &second.current_scene));
    assert!(!Arc::ptr_eq(&first.statistics, &second.statistics));

    let second_scene = second.get_scene().await;
    let mut first_scene = first.get_scene().await;
    first_scene.id = "first-only".into();
    first.set_scene(first_scene).await.unwrap();
    first.update_statistics(10.0).await;
    assert_eq!(second.get_scene().await.id, second_scene.id);
    assert_eq!(second.get_statistics().await.frame_time_ms, 0.0);
    assert_eq!(first.get_scene().await.id, "first-only");
    assert_eq!(first.get_statistics().await.fps, 100.0);
}
