//! Direct frame coverage for admitted objects and retail startup.

use crate::game_logic::{
    DEFAULT_SKIRMISH_MAP_NAME, GameLogic, GameMode, KindOf, LONE_EAGLE_MAP_NAME, Team,
    ThingTemplate, resolve_retail_map_path,
};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

#[test]
fn receiving_frame_reports_a_created_object_by_its_id() {
    let mut logic = GameLogic::new();
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(before.objects.is_empty());

    let mut template = ThingTemplate::new("FrameRanger");
    template.set_health(120.0);
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Selectable);
    logic.templates.insert("FrameRanger".into(), template);
    let position = Vec3::new(8.0, 0.0, 16.0);
    let id = logic
        .create_object("FrameRanger", Team::USA, position)
        .unwrap();
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(frame.renderable_object_ids(), [id]);
    let row = &frame.objects[0];
    assert_eq!(row.template_name, "FrameRanger");
    assert_eq!(row.position, position);
    assert_eq!(row.health_current, 120.0);
    assert!(before.objects.is_empty());
}

#[test]
fn retail_startup_publishes_the_receiving_world_after_its_first_step() {
    let map = [LONE_EAGLE_MAP_NAME, DEFAULT_SKIRMISH_MAP_NAME]
        .into_iter()
        .find_map(|name| resolve_retail_map_path(name).map(|path| (name, path)));
    let Some((name, path)) = map else {
        eprintln!("Retail maps unavailable for the startup publication case");
        return;
    };
    let mut logic = GameLogic::new();
    crate::cnc_game_engine::start_standalone_game(&mut logic, GameMode::Skirmish).unwrap();
    apply_skirmish_config(&mut logic, &golden_skirmish_config("ReceivingRetailFrame")).unwrap();
    assert!(logic.load_map(&path.to_string_lossy()) || logic.load_map(name));
    let before = logic.get_frame();
    logic.update();
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(frame.frame.0, before + 1);
    assert!(!frame.objects.is_empty());
    for row in &frame.objects {
        assert!(logic.host_objects().lookup_registered(&row.id).is_some());
    }
}
