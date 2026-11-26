#[test]
fn retail_maps_advance_and_publish_objects() {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;

    let map_names = [DEFAULT_SKIRMISH_MAP_WAVE169, LONE_EAGLE_MAP_WAVE169];
    let [defcon_path, lone_path] = map_names.map(resolve_retail_map_path);
    if defcon_path.is_none() && lone_path.is_none() {
        eprintln!("Retail Defcon6 and Lone Eagle maps are unavailable");
        return;
    }
    let paths = [
        defcon_path.expect("retail maps install must contain Defcon6"),
        lone_path.expect("retail maps install must contain Lone Eagle"),
    ];
    for (name, path) in map_names.into_iter().zip(paths) {
        let mut logic = GameLogic::new();
        crate::cnc_game_engine::start_standalone_game(&mut logic, GameMode::Skirmish)
            .expect("admit retail skirmish definitions");
        assert!(
            logic.load_map(&path.to_string_lossy()) || logic.load_map(name),
            "load {name}",
        );
        let initial_frame = logic.get_frame();
        for _ in 0..5 {
            logic.update();
        }
        assert!(logic.get_frame() > initial_frame, "{name} must advance");
        let mut frame = PresentationFrame::build_from_logic(&mut logic, 0);
        assert_eq!(frame.frame.0, logic.get_frame(), "{name} frame");

        if name == LONE_EAGLE_MAP_WAVE169 {
            assert!(!frame.objects.is_empty(), "{name} must publish objects");
            let mut shadow = GameWorldShadow::new(4096);
            shadow.sync_from_host(&logic);
            let probe = shadow.probe(&logic);
            assert!(probe.host_objects > 0 && probe.shadow_entities > 0);
            assert!(probe.mapped_objects > 0);
            frame.overlay_gameworld_shadow(&shadow);
            assert!(
                frame
                    .objects
                    .iter()
                    .any(|row| shadow.entity_for_host(row.id).is_some()),
                "{name} must retain mapped object identities after the overlay",
            );
        }
    }
}
