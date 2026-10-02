use super::*;

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn callbacks_run_once_in_order_on_the_driving_thread_even_after_errors() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .expect("headless engine");
    let mut renderer = WgpuMainRenderer::from_engine(Default::default()).expect("renderer");
    WW3D::with_renderer(|backend| {
        assert!(backend.is_sorting_enabled());
        assert!(backend.are_static_sort_lists_enabled());
        assert!(backend.are_decals_enabled());
        Ok(())
    })
    .expect("registered backend")
    .expect("initial switches");
    for enabled in [false, true] {
        WW3D::enable_sorting(enabled).expect("sorting switch");
        WW3D::set_static_sort_lists_enabled(enabled).expect("static sorting switch");
        WW3D::set_decals_enabled(enabled).expect("decal switch");
        WW3D::with_renderer(|backend| {
            assert_eq!(backend.is_sorting_enabled(), enabled);
            assert_eq!(backend.are_static_sort_lists_enabled(), enabled);
            assert_eq!(backend.are_decals_enabled(), enabled);
            Ok(())
        })
        .expect("registered backend")
        .expect("switch getters");
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let driver = std::thread::current().id();
    for index in 0..2 {
        let pre_calls = calls.clone();
        renderer.enqueue_pre_scene_callback(move |_| {
            pre_calls
                .lock()
                .unwrap()
                .push(("pre", index, std::thread::current().id()));
            Ok(())
        });
        let post_calls = calls.clone();
        renderer.enqueue_post_frame_callback(move |_| {
            post_calls
                .lock()
                .unwrap()
                .push(("post", index, std::thread::current().id()));
            if index == 0 {
                Err(RendererError::InvalidOperation(
                    "expected callback failure".into(),
                ))
            } else {
                Ok(())
            }
        });
    }
    assert!(calls.lock().unwrap().is_empty());
    renderer.begin_frame().expect("begin");
    assert!(calls.lock().unwrap().is_empty());
    renderer
        .end_frame()
        .expect("post errors do not abort later callbacks");
    let expected = vec![
        ("pre", 0, driver),
        ("pre", 1, driver),
        ("post", 0, driver),
        ("post", 1, driver),
    ];
    assert_eq!(*calls.lock().unwrap(), expected);
    renderer.begin_frame().expect("second begin");
    renderer.end_frame().expect("second end");
    assert_eq!(
        *calls.lock().unwrap(),
        expected,
        "callbacks are consumed once"
    );
    let calls_after_error = calls.clone();
    renderer.enqueue_pre_scene_callback(move |_| {
        calls_after_error
            .lock()
            .unwrap()
            .push(("pre error", 0, std::thread::current().id()));
        Err(RendererError::InvalidOperation(
            "expected pre-scene failure".into(),
        ))
    });
    renderer
        .enqueue_pre_scene_callback(|_| panic!("pre-scene failure must stop later pre callbacks"));
    let calls_after_error = calls.clone();
    renderer.enqueue_post_frame_callback(move |_| {
        calls_after_error.lock().unwrap().push((
            "post after error",
            0,
            std::thread::current().id(),
        ));
        Ok(())
    });
    renderer.begin_frame().expect("third begin");
    assert!(renderer.end_frame().is_err());
    assert_eq!(
        &calls.lock().unwrap()[4..],
        &[("pre error", 0, driver), ("post after error", 0, driver)]
    );
    assert!(!ww3d_engine::frame_is_active(), "failed frame still ends");
    drop(renderer);
    ww3d_engine::shutdown().expect("shutdown");
}
