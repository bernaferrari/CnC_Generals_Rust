use super::*;
fn visual(raw: u8) -> game_client::terrain::terrain_visual::TerrainVisualImpl {
    let mut map = game_client::terrain::height_map::HeightMap::new(8, 8, 159.375, 10.);
    map.heights.fill(f32::from(raw) / 255.);
    let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    visual.load_heightmap_from_data(map, None, None).unwrap();
    visual
}
#[test]
#[ignore = "requires native GPU for actual RenderPipeline construction"]
fn two_actual_pipeline_terrain_owners_accept_canonical_bytes_without_global_selection() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    let mut a = RenderPipeline::initialize(&graphics).unwrap();
    assert!(
        a.native_terrain().is_none(),
        "pipeline construction is inert"
    );
    a.native_terrain = Some(visual(40));
    let mut b = RenderPipeline::initialize(&graphics).unwrap();
    assert!(b.native_terrain().is_none());
    assert_eq!(a.native_terrain().unwrap().get_raw_map_height(1, 1), 40);
    b.native_terrain = Some(visual(80));
    let mut logical = gamelogic::terrain::TerrainLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 8;
    map.height = 8;
    map.heightmap = vec![40; 64];
    logical.try_load_map_geometry(map).unwrap();
    // Hold the published compatibility visual unavailable for the whole effect.
    let global = game_client::terrain::terrain_visual::get_terrain_visual().unwrap();
    logical.set_raw_map_height_with_effects(1, 1, -1, &mut |change| {
        a.native_terrain_mut().unwrap().apply_accepted_height(
            change.sample_x,
            change.sample_y,
            change.raw_height,
        );
    });
    assert_eq!(logical.logic_height_map_bytes()[9], 255);
    assert_eq!(
        a.native_terrain().unwrap().get_raw_map_height(1, 1),
        255,
        "accepted Int narrowing is not clamped/recompared by native presentation"
    );
    assert_eq!(b.native_terrain().unwrap().get_raw_map_height(1, 1), 80);
    b.clear_native_terrain();
    assert_eq!(a.native_terrain().unwrap().get_raw_map_height(1, 1), 255);
    drop(global);
    assert!(
        a.native_terrain_mut()
            .unwrap()
            .add_scorch([1., 2., 3.], 5., 0)
    );
    a.hydrate_native_terrain_gpu(&graphics.device_arc(), &graphics.queue_arc())
        .unwrap();
    assert_eq!(
        a.native_terrain().unwrap().get_raw_map_height(1, 1),
        255,
        "GPU hydration must not replace the accepted CPU height map"
    );
    assert_eq!(
        a.native_terrain().unwrap().scorch_marks().len(),
        1,
        "GPU hydration must retain the saved/authored scorch owner"
    );
    drop(a);
    drop(b);
    drop(graphics);
    ww3d_engine::shutdown().unwrap();
}
