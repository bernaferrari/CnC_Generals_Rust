#[test]
fn retail_maps_advance_and_publish_objects() {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
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
        let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        assert_eq!(frame.frame.0, logic.get_frame(), "{name} frame");

        if name == LONE_EAGLE_MAP_WAVE169 {
            assert!(!frame.objects.is_empty(), "{name} must publish objects");
            for row in &frame.objects {
                let object = logic
                    .host_objects()
                    .lookup_registered(&row.id)
                    .expect("published retail actor");
                assert_eq!(row.template_name, object.template_name);
            }
        }
    }
}
