#![cfg(all(feature = "game_client", feature = "integration-tests"))]

use game_client::core::PresentationDirectDrawableBindingKey;
use game_client::core::game_client::{
    NativeAuxiliaryDrawObservation, NativeAuxiliaryReceipt, NativeDrawCandidate,
};
use game_client::display::view_filter::ViewFilterState;
use game_client::render_bridge::RenderBridge;
use game_client::snow::{SnowManager, SnowRenderState, WeatherSetting};
use game_client::terrain::terrain_visual::{TerrainVisualImpl, TerrainVisualMotion};
use generals_main::graphics::GraphicsSystem;
use generals_main::graphics::render_pipeline::{
    FilterCameraMatrices, FrozenDirectDrawableSceneCandidate, FrozenDirectDrawableSceneDecision,
    NativeFrameRenderReceiver, NativeInstalledPoseLoan, RenderPipeline,
};
use generals_main::presentation_frame::{PresentationFrame, UnitRenderInput};
use glam::{Mat4, Vec2, Vec3};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Receiver {
    fail_collect: bool,
    filter: ViewFilterState,
}

impl NativeFrameRenderReceiver for Receiver {
    fn actor_draw_candidate_is_current(
        &self,
        _: PresentationDirectDrawableBindingKey,
        _: &UnitRenderInput,
    ) -> Result<bool, String> {
        Ok(true)
    }
    fn native_draw_candidates(&mut self) -> Result<Vec<NativeDrawCandidate>, String> {
        if self.fail_collect {
            Err("injected native collection failure".into())
        } else {
            Ok(Vec::new())
        }
    }
    fn observe_auxiliary_drawable(
        &mut self,
        _: &NativeAuxiliaryReceipt,
    ) -> Result<Option<NativeAuxiliaryDrawObservation>, String> {
        Ok(None)
    }
    fn complete_physics_visual(
        &mut self,
        _: &mut UnitRenderInput,
        _: &PresentationFrame,
    ) -> Result<Option<Mat4>, String> {
        Ok(None)
    }
    fn direct_scene_decisions(
        &mut self,
        _: &[FrozenDirectDrawableSceneCandidate],
    ) -> Vec<FrozenDirectDrawableSceneDecision> {
        Vec::new()
    }
    fn view_filter(&self) -> &ViewFilterState {
        &self.filter
    }
    fn view_filter_mut(&mut self) -> &mut ViewFilterState {
        &mut self.filter
    }
    fn complete_filter_camera(
        &mut self,
        _: Option<Vec3>,
        _: &mut NativeInstalledPoseLoan<'_>,
        _: &mut GraphicsSystem,
    ) -> Result<FilterCameraMatrices, String> {
        Ok(FilterCameraMatrices {
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
            position: Vec3::ZERO,
        })
    }
    fn complete_tree_cpu_draw(
        &mut self,
        _: &mut TerrainVisualImpl,
        _: NativeInstalledPoseLoan<'_>,
        _: &mut GraphicsSystem,
        _: TerrainVisualMotion,
    ) -> Result<Option<game_client::terrain::TreeCpuDrawReceipt>, String> {
        Ok(None)
    }
}

fn execute(
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    receiver: &mut Receiver,
) -> anyhow::Result<()> {
    let mut bridge = RenderBridge::new();
    let snow = SnowRenderState {
        manager: SnowManager::new(),
        definition: WeatherSetting::default(),
    };
    pipeline.execute(
        graphics,
        &mut bridge,
        &Mat4::IDENTITY,
        &Mat4::IDENTITY,
        Vec3::ZERO,
        0.0,
        false,
        0,
        false,
        receiver,
        Vec::new(),
        None,
        &snow,
        Vec2::ZERO,
        0,
        None,
    )
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --ignored"]
fn failed_collection_discards_only_its_frame_work_before_retry() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    // One guard for these reads: separate lock() temporaries in one call
    // keep the first guard until the statement ends (non-reentrant Mutex).
    let (device, queue, color_format, depth_format) = {
        let engine = engine.lock();
        (
            engine.device(),
            engine.queue(),
            engine.color_format(),
            engine.depth_format(),
        )
    };
    let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
        .unwrap();
    let mut owner = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut world = generals_main::game_logic::GameLogic::new();
    let frame = std::rc::Rc::new(PresentationFrame::build_from_logic(&mut world, 0));
    owner.set_presentation_frame(Some(frame.clone()));
    peer.set_presentation_frame(Some(frame));
    let stale = Arc::new(AtomicUsize::new(0));
    let peer_calls = Arc::new(AtomicUsize::new(0));
    let stale_capture = stale.clone();
    owner.enqueue_frame_work(ww3d_renderer_3d::rendering::wgpu_main_renderer::FramePhase::Overlay2D, move |_| {
        stale_capture.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let peer_capture = peer_calls.clone();
    peer.enqueue_frame_work(ww3d_renderer_3d::rendering::wgpu_main_renderer::FramePhase::Overlay2D, move |_| {
        peer_capture.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let mut failing = Receiver {
        fail_collect: true,
        ..Default::default()
    };
    assert!(
        execute(&mut owner, &mut graphics, &mut failing)
            .unwrap_err()
            .to_string()
            .contains("injected native collection failure")
    );
    execute(&mut peer, &mut graphics, &mut Receiver::default()).unwrap();
    let fresh = Arc::new(AtomicUsize::new(0));
    let fresh_capture = fresh.clone();
    owner.enqueue_frame_work(ww3d_renderer_3d::rendering::wgpu_main_renderer::FramePhase::Overlay2D, move |_| {
        fresh_capture.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    execute(&mut owner, &mut graphics, &mut Receiver::default()).unwrap();
    assert_eq!(
        stale.load(Ordering::SeqCst),
        0,
        "failed frame callback must not run on retry"
    );
    assert_eq!(
        peer_calls.load(Ordering::SeqCst),
        1,
        "another session retains its callback"
    );
    assert_eq!(
        fresh.load(Ordering::SeqCst),
        1,
        "retry runs its own callback"
    );
    drop(peer);
    drop(owner);
    drop(engine);
}
