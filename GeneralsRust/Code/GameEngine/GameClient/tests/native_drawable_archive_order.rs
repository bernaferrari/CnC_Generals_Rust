use game_client_rust::core::game_client::{GameClient, ReceivingDrawablePolicy};
use game_client_rust::drawable::drawable::BasicDrawable;
use game_client_rust::drawable::drawable::DrawableId;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module::StaticGameLodLevel;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

fn roster(client: &GameClient) -> Vec<DrawableId> {
    let mut ids = Vec::new();
    client
        .iterate_drawables_in_region(None, |drawable| ids.push(drawable.get_id()))
        .unwrap();
    ids
}

#[test]
fn xfer_follows_the_client_list_and_rebuilds_it_by_constructor_prepend() {
    let mut source = GameClient::new().unwrap();
    for _ in 0..3 {
        source
            .register_drawable_with_template(
                Box::new(BasicDrawable::new(DrawableId::INVALID)),
                Some("ArchiveUnit".into()),
            )
            .unwrap();
    }
    assert_eq!(
        roster(&source),
        [DrawableId(3), DrawableId(2), DrawableId(1)]
    );
    let bytes = source.capture_xfer_bytes().unwrap();

    let factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
    let mut content = ReceivingModuleContent::new(factory);
    content
        .load_ini_text("Object ArchiveUnit\nEnd\n", INILoadType::Overwrite)
        .unwrap();
    let mut target = GameClient::new().unwrap();
    target
        .restore_from_xfer_bytes_with_receiving_content(
            &bytes,
            &mut content,
            ReceivingDrawablePolicy {
                time_of_day: None,
                use_draw_module_lod: true,
                static_game_lod: StaticGameLodLevel::High,
                use_tree_sway: false,
            },
        )
        .unwrap();
    // C++ saves list order 3,2,1 and prepends each newly loaded Drawable.
    assert_eq!(
        roster(&target),
        [DrawableId(1), DrawableId(2), DrawableId(3)]
    );

    // A later constructor produces a nonmonotonic roster. Its next archive
    // must keep visitation order rather than sorting the saved IDs.
    let id = target
        .register_drawable_with_template(
            Box::new(BasicDrawable::new(DrawableId::INVALID)),
            Some("ArchiveUnit".into()),
        )
        .unwrap();
    assert!(![DrawableId(1), DrawableId(2), DrawableId(3)].contains(&id));
    assert_eq!(
        roster(&target),
        [id, DrawableId(1), DrawableId(2), DrawableId(3)]
    );
    let bytes = target.capture_xfer_bytes().unwrap();
    let mut next = GameClient::new().unwrap();
    next.restore_from_xfer_bytes_with_receiving_content(
        &bytes,
        &mut content,
        ReceivingDrawablePolicy {
            time_of_day: None,
            use_draw_module_lod: true,
            static_game_lod: StaticGameLodLevel::High,
            use_tree_sway: false,
        },
    )
    .unwrap();
    assert_eq!(
        roster(&next),
        [DrawableId(3), DrawableId(2), DrawableId(1), id]
    );
}
