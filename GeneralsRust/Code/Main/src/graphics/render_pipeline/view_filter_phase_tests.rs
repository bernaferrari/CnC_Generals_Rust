//! Native GPU-phase witness, not a rendered match or pixel parity claim.
use super::*;
use game_client::display::view::{FilterMode, FilterType, Point3, Vector2};
use game_client::display::view_filter::ViewFilterState;

#[test]
#[ignore = "requires native WGPU; run explicit, isolated GPU acceptance"]
fn borrowed_filter_finishes_on_current_renderer_frame_and_peak_is_not_replayed() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("native filter acceptance requires a headless device");
    struct EngineScope;
    impl Drop for EngineScope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = EngineScope;
    let mut renderer = WgpuMainRenderer::from_engine(WgpuMainRendererConfig::default()).unwrap();
    let graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut logic = crate::game_logic::GameLogic::new();
    let presentation =
        Rc::new(crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 0));
    let mut filter = ViewFilterState::new();
    filter.set_filter(FilterType::MotionBlur);
    filter.set_mode(FilterMode::MBInAndOutAlpha);
    filter.set_pos(Point3::new(10., 20., 30.));
    let peer = ViewFilterState::new();
    let mut completions = 0;
    let mut peaks = Vec::new();
    for logic_frame in 1..=13 {
        for offer in 0..4 {
            pipeline.set_presentation_frame(Some(Rc::clone(&presentation)));
            let resources = &mut pipeline.view_filter_resources;
            if logic_frame > 1 || offer > 0 {
                assert!(
                    resources.has_scene_capture_for(32, 32, ww3d_engine::color_format().unwrap()),
                    "ordinary production frame handoff must retain the prior capture"
                );
            }
            let skip_scene = filter.skip_render()
                && resources.has_scene_capture_for(32, 32, ww3d_engine::color_format().unwrap());
            renderer.begin_frame().unwrap();
            let before = completions;
            let mut finish = |frame: &mut ww3d_engine::RenderFrame| {
                let dest = frame.color_view_arc();
                let device = frame.device_arc();
                let queue = frame.queue_arc();
                let format = frame.color_format();
                let (texture, encoder) = frame.color_texture_and_encoder();
                let composite = filter.composite(Vector2::zero());
                assert!(resources.prepare(&device, &queue, format, 32, 32, &composite));
                let step = filter.post_render(logic_frame, Vector2::zero());
                assert_eq!(
                    step.motion_blur_count,
                    if logic_frame <= 12 {
                        logic_frame as i32 * 5
                    } else {
                        55
                    }
                );
                if let Some(point) = step.look_at {
                    peaks.push((logic_frame, offer, point));
                }
                assert!(resources.composite_retained_view_filter(
                    &device,
                    &queue,
                    encoder,
                    &dest,
                    texture,
                    format,
                    32,
                    32,
                    &composite,
                    &step,
                    !skip_scene,
                ));
                completions += 1;
                Ok(())
            };
            renderer
                .end_frame_with_scene_policy(skip_scene, Some(&mut finish))
                .unwrap();
            assert_eq!(
                completions,
                before + 1,
                "borrowed completion belongs to this offer, not the next one"
            );
        }
    }
    assert_eq!(peaks, vec![(12, 0, Point3::new(10., 20., 30.))]);
    assert_eq!(peer.filter(), FilterType::Null);
    assert_eq!(completions, 52);
    pipeline.invalidate_world_visual_state();
    assert!(
        !pipeline.view_filter_resources.has_scene_capture_for(
            32,
            32,
            ww3d_engine::color_format().unwrap()
        ),
        "world replacement must invalidate another world's capture"
    );
}
