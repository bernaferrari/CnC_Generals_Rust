use generals_main::fow_rendering::{
    PresentationFowGrid, ProjectedShroudMetadata, ProjectedShroudSnapshot,
};

#[cfg(feature = "game_client")]
#[test]
fn projected_shroud_enable_disable_border_override_is_frozen() {
    use generals_main::game_logic::mission_scripts::{MissionScriptActionHandler, MissionScriptHooks};
    use gamelogic::scripting::engine::ScriptActionHandler;
    let hooks = MissionScriptHooks::new();
    let peer = MissionScriptHooks::new();
    let mut global = game_engine::common::global_data::GlobalData::default();
    global.shroud_alpha = 37;
    global.clear_alpha = 211;
    let grid = PresentationFowGrid::fully_visible(3, 2, 10.0);
    let handler = MissionScriptActionHandler::new(hooks.clone());

    // C++ DisableBorderShroud dispatches the configured ClearAlpha to the
    // receiving script hook. Freeze its override into the complete padded R8
    // allocation before later script state can change.
    handler
        .set_border_shroud_level(global.clear_alpha)
        .expect("DisableBorderShroud display handoff");
    let level = hooks.drain_border_shroud_levels();
    assert_eq!(level, [211]);
    assert!(peer.drain_border_shroud_levels().is_empty());
    let disabled = ProjectedShroudSnapshot::from_grid(
        &grid,
        ProjectedShroudMetadata::from_global_data_with_border_override(
            &global,
            level.last().copied(),
        ),
    );
    assert_eq!(disabled.metadata.border_alpha, 211);
    assert_eq!(disabled.texel_at(0, 0), Some(211));
    assert_eq!(disabled.texel_at(7, 3), Some(211));

    // C++ EnableBorderShroud sends ShroudAlpha.  A fresh snapshot sees the
    // new border while the prior presentation frame stays fully frozen.
    handler
        .set_border_shroud_level(global.shroud_alpha)
        .expect("EnableBorderShroud display handoff");
    let level = hooks.drain_border_shroud_levels();
    assert_eq!(level, [37]);
    let enabled = ProjectedShroudSnapshot::from_grid(
        &grid,
        ProjectedShroudMetadata::from_global_data_with_border_override(
            &global,
            level.last().copied(),
        ),
    );
    assert_eq!(enabled.metadata.border_alpha, 37);
    assert_eq!(enabled.texel_at(0, 0), Some(37));
    assert_eq!(disabled.metadata.border_alpha, 211);
    assert_eq!(disabled.texel_at(0, 0), Some(211));
}
