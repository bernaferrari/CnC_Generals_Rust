use crate::cnc_game_engine::CnCGameEngine;
use crate::game_logic::{GameLogic, Team, ThingTemplate};
use crate::presentation_frame::PresentationFrame;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

fn admit(world: &mut GameLogic, modules: &str) {
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                &format!("Object ReceivingDrawNames\n{modules}\nEnd\n"),
                "receiving-draw-names.ini",
            )
            .unwrap(),
        1,
    );
    let definition = parser.get_definition("ReceivingDrawNames").unwrap();
    let mut template = ThingTemplate::new("ReceivingDrawNames");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    world
        .templates
        .insert("ReceivingDrawNames".into(), template);
}
fn names(
    frame: &PresentationFrame,
    cache: &RefCell<HashMap<String, Arc<Vec<String>>>>,
) -> Arc<Vec<String>> {
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|direct| direct.visual_template_name == "ReceivingDrawNames")
        .expect("actual frozen direct Drawable receiver");
    CnCGameEngine::presentation_draw_module_names_from_template(
        cache,
        &direct.visual_template_name,
        Some(
            direct
                .receiving_visual_draw_models
                .as_deref()
                .expect("admitted Draw definition"),
        ),
        None,
    )
}
#[test]
fn frozen_draw_classes_use_receiving_definition_without_application_lookup() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "frozen_draw_classes_use_receiving_definition_without_application_lookup",
        || {
            let mut first = GameLogic::new();
            let mut peer = GameLogic::new();
            admit(
                &mut first,
                "Draw = W3DTruckDraw TruckTag\nDefaultConditionState\nModel = None\nEnd\nEnd\nDraw = W3DDefaultDraw DefaultTag\nEnd",
            );
            admit(
                &mut peer,
                "Draw = W3DTankDraw TankTag\nDefaultConditionState\nModel = None\nEnd\nEnd",
            );
            let first_id = first
                .create_object("ReceivingDrawNames", Team::USA, glam::Vec3::ZERO)
                .unwrap();
            let peer_id = peer
                .create_object("ReceivingDrawNames", Team::USA, glam::Vec3::ZERO)
                .unwrap();
            assert_eq!(first_id, peer_id);
            let first_frame = PresentationFrame::build_from_logic(&mut first, 0);
            let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
            let cache = RefCell::new(HashMap::from([(
                "ReceivingDrawNames".into(),
                Arc::new(vec!["ForeignApplicationClass".into()]),
            )]));
            let frozen_names = names(&first_frame, &cache);
            assert_eq!(&*frozen_names, &["W3DTruckDraw", "W3DDefaultDraw"]);
            assert!(Arc::ptr_eq(&frozen_names, &names(&first_frame, &cache)));
            assert_eq!(&*names(&peer_frame, &cache), &["W3DTankDraw"]);
            // Later receiving admission cannot change the already frozen source Arc.
            admit(
                &mut first,
                "Draw = W3DModelDraw ReplacementTag\nDefaultConditionState\nModel = None\nEnd\nEnd",
            );
            assert_eq!(
                &*first.templates["ReceivingDrawNames"]
                    .receiving_draw_models()
                    .unwrap()
                    .module_class_names(),
                &["W3DModelDraw"]
            );
            assert_eq!(
                &*names(&first_frame, &cache),
                &["W3DTruckDraw", "W3DDefaultDraw"]
            );
            assert!(Arc::ptr_eq(&frozen_names, &names(&first_frame, &cache)));
            assert_eq!(&*names(&peer_frame, &cache), &["W3DTankDraw"]);
            assert_eq!(
                &*cache.borrow()["ReceivingDrawNames"],
                &["ForeignApplicationClass"]
            );
        },
    );
}
#[test]
fn authored_empty_draw_classes_suppress_a_populated_application_cache() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_empty_draw_classes_suppress_a_populated_application_cache",
        || {
            let mut world = GameLogic::new();
            admit(&mut world, "");
            world
                .create_object("ReceivingDrawNames", Team::USA, glam::Vec3::ZERO)
                .unwrap();
            let frame = PresentationFrame::build_from_logic(&mut world, 0);
            let cache = RefCell::new(HashMap::from([(
                "ReceivingDrawNames".into(),
                Arc::new(vec!["W3DTruckDraw".into()]),
            )]));
            let empty_names = names(&frame, &cache);
            assert!(empty_names.is_empty());
            assert!(Arc::ptr_eq(&empty_names, &names(&frame, &cache)));
            assert_eq!(&*cache.borrow()["ReceivingDrawNames"], &["W3DTruckDraw"]);
        },
    );
}
#[test]
fn unadmitted_curated_draw_names_keep_the_existing_application_cache() {
    let admitted = Arc::new(vec!["W3DTruckDraw".into()]);
    let cache = RefCell::new(HashMap::from([("Curated".into(), Arc::clone(&admitted))]));
    let selected =
        CnCGameEngine::presentation_draw_module_names_from_template(&cache, "Curated", None, None);
    assert!(Arc::ptr_eq(&selected, &admitted));
}
