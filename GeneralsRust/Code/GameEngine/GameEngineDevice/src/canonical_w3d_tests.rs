use super::*;

fn headless_config() -> DeviceConfig {
    DeviceConfig::w3d()
        .with_parameter("width", 128)
        .with_parameter("height", 128)
        .with_parameter("msaa_samples", 1)
}

#[tokio::test]
async fn repeated_admission_returns_one_owner_through_shutdown_and_reinit() {
    let system = GameEngineDevice::new().await.unwrap();
    let first = system.init_w3d_device(headless_config()).await.unwrap();
    let second = system.init_w3d_device(headless_config()).await.unwrap();
    assert!(first.get_status().await.unwrap().initialized);

    let mut scene = first.get_scene().await;
    scene.id = "admitted-owner".into();
    first.set_scene(scene).await.unwrap();
    let mut shader = first.get_shader("w3d_default_pbr").await.unwrap();
    shader.id = "owner-only-shader".into();
    first.add_shader(shader).await.unwrap();
    assert!(second.get_shader("owner-only-shader").await.is_some());
    second.update_statistics(25.0).await;
    assert_eq!(second.get_scene().await.id, "admitted-owner");
    assert_eq!(first.get_statistics().await.fps, 40.0);
    assert!(system.get_system_status().await.unwrap()[0].initialized);

    system.shutdown().await.unwrap();
    assert!(system.get_system_status().await.unwrap().is_empty());
    assert!(!first.get_status().await.unwrap().initialized);
    assert!(!second.get_status().await.unwrap().initialized);
    assert!(first.get_shader("owner-only-shader").await.is_none());
    assert!(second.get_shader("owner-only-shader").await.is_none());
    // Existing handles keep the old, shut-down state alive.
    assert_eq!(second.get_scene().await.id, "admitted-owner");

    let restored = system.init_w3d_device(headless_config()).await.unwrap();
    assert!(restored.get_status().await.unwrap().initialized);
    assert!(!Arc::ptr_eq(&first, &restored));
    assert_ne!(restored.get_scene().await.id, "admitted-owner");
    assert_eq!(restored.get_statistics().await.frame_time_ms, 0.0);
    assert!(restored.get_shader("owner-only-shader").await.is_none());
    assert!(restored.get_shader("w3d_default_pbr").await.is_some());
    system.shutdown().await.unwrap();

    // The manager must publish its actual owner, rather than a new wrapper per admission.
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn independently_admitted_devices_keep_same_named_scene_state_isolated() {
    let first_system = GameEngineDevice::new().await.unwrap();
    let second_system = GameEngineDevice::new().await.unwrap();
    let first = first_system
        .init_w3d_device(headless_config())
        .await
        .unwrap();
    let second = second_system
        .init_w3d_device(headless_config())
        .await
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &second));

    let mut first_scene = first.get_scene().await;
    first_scene.id = "same-scene-id".into();
    first_scene.background_color = [0.2, 0.3, 0.4, 1.0];
    let mut second_scene = second.get_scene().await;
    second_scene.id = first_scene.id.clone();
    second_scene.background_color = [0.5, 0.6, 0.7, 1.0];
    first.set_scene(first_scene).await.unwrap();
    second.set_scene(second_scene).await.unwrap();
    first.update_statistics(10.0).await;
    second.update_statistics(20.0).await;
    assert_eq!(
        first.get_scene().await.background_color,
        [0.2, 0.3, 0.4, 1.0]
    );
    assert_eq!(
        second.get_scene().await.background_color,
        [0.5, 0.6, 0.7, 1.0]
    );
    assert_eq!(first.get_statistics().await.fps, 100.0);
    assert_eq!(second.get_statistics().await.fps, 50.0);

    first_system.shutdown().await.unwrap();
    assert!(!first.get_status().await.unwrap().initialized);
    assert!(second.get_status().await.unwrap().initialized);
    assert_eq!(second.get_statistics().await.fps, 50.0);
    second_system.shutdown().await.unwrap();
}
