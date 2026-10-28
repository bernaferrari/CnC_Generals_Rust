use game_client_rust::core::game_client::{GameClient, ReceivingDrawablePolicy};
use game_client_rust::drawable::BasicDrawable;
use game_engine::common::{
    ini::INILoadType,
    name_key_generator::NameKeyNamespace,
    thing::{
        module::StaticGameLodLevel, module_factory::ModuleFactory,
        receiving_module_content::ReceivingModuleContent,
    },
};
fn content(namespace_prefix: &str) -> ReceivingModuleContent {
    let mut modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    if !namespace_prefix.is_empty() {
        modules
            .receiving_name_keys_mut()
            .unwrap()
            .name_to_key(namespace_prefix);
    }
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut modules).unwrap();
    let mut content = ReceivingModuleContent::new(modules);
    content
        .load_ini_text(
            "Object RestoreReceivingTemplate\nBuildCost = 50\nDraw = W3DTankDraw TankTag\nTreadAnimationRate = 1.5\nEnd\nEnd",
            INILoadType::Overwrite,
        )
        .unwrap();
    content
}

fn override_build_cost(base: &ReceivingModuleContent, build_cost: u32) -> ReceivingModuleContent {
    let mut content = base.clone();
    content
        .load_ini_text(
            &format!("Object RestoreReceivingTemplate\nBuildCost = {build_cost}\nEnd"),
            INILoadType::CreateOverrides,
        )
        .unwrap();
    content
}

fn policy() -> ReceivingDrawablePolicy {
    ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Afternoon),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    }
}

fn restored_tank_module_key(
    client: &mut GameClient,
    drawable_id: game_client_rust::drawable::DrawableId,
) -> u32 {
    let drawable = client.find_drawable_by_id_mut(drawable_id).unwrap();
    let basic = drawable
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    let module = basic.get_draw_modules_mut().first_mut().unwrap();
    assert_eq!(module.snapshot_module_identifier(), Some("TankTag"));
    let module = module.logic_module_mut().unwrap();
    let data = module
        .get_module_data()
        .as_any()
        .downcast_ref::<gamelogic::object::draw::W3DTankDrawModuleData>()
        .unwrap();
    assert_eq!(data.tread_animation_rate, 1.5_f32 * (1.0_f32 / 30.0));
    module.get_module_tag_name_key()
}

fn public_xfer_restore_uses_each_receiving_map_override_and_module_factory() {
    let base_a = content("");
    let base_b = content("PeerNamespacePrefix");
    let source_factory = base_a.compatibility_template_view();
    let source_template = source_factory
        .find_template("RestoreReceivingTemplate", false)
        .unwrap();
    let previous_factory = game_engine::common::thing::get_thing_factory()
        .unwrap()
        .replace(source_factory);
    let mut source = GameClient::new().unwrap();
    let drawable_id = source
        .create_drawable_from_template(&source_template)
        .unwrap();
    let bytes = source.capture_xfer_bytes().unwrap();
    *game_engine::common::thing::get_thing_factory().unwrap() = previous_factory;

    let mut world_a_content = override_build_cost(&base_a, 101);
    let mut world_b_content = override_build_cost(&base_b, 202);
    let resolved_a = world_a_content
        .resolve_template("RestoreReceivingTemplate")
        .unwrap();
    let resolved_b = world_b_content
        .resolve_template("RestoreReceivingTemplate")
        .unwrap();
    assert_eq!(
        resolved_a.effective_admission().template().get_build_cost(),
        101
    );
    assert_eq!(
        resolved_b.effective_admission().template().get_build_cost(),
        202
    );
    let expected_a = world_a_content
        .module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .name_to_key("TankTag");
    let expected_b = world_b_content
        .module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .name_to_key("TankTag");
    assert_ne!(expected_a, expected_b);

    let mut world_a = GameClient::new().unwrap();
    let mut world_b = GameClient::new().unwrap();
    world_a
        .restore_from_xfer_bytes_with_receiving_content(&bytes, &mut world_a_content, policy())
        .unwrap();
    world_b
        .restore_from_xfer_bytes_with_receiving_content(&bytes, &mut world_b_content, policy())
        .unwrap();
    assert_eq!(
        restored_tank_module_key(&mut world_a, drawable_id),
        expected_a
    );
    assert_eq!(
        restored_tank_module_key(&mut world_b, drawable_id),
        expected_b
    );
}

#[test]
fn public_native_restore_uses_receiving_color_and_does_not_import_same_id_registry_drawable() {
    // Both source fixtures temporarily install the compatibility ThingFactory.
    // Keep them in one test process without a shared test lock.
    public_xfer_restore_uses_each_receiving_map_override_and_module_factory();
    use gamelogic::object::drawable::DrawableExt as _;
    const OBJECT_ID: u32 = 840_219;
    const FOREIGN_DRAWABLE_ID: u32 = 991;
    let admitted = content("");
    let source_factory = admitted.compatibility_template_view();
    let source_template = source_factory
        .find_template("RestoreReceivingTemplate", false)
        .unwrap();
    let previous_factory = game_engine::common::thing::get_thing_factory()
        .unwrap()
        .replace(source_factory);
    let mut source = GameClient::new().unwrap();
    let drawable_id = source
        .create_drawable_from_template(&source_template)
        .unwrap();
    source
        .bind_drawable_to_object(drawable_id, OBJECT_ID)
        .unwrap();
    let bytes = source.capture_xfer_bytes().unwrap();
    *game_engine::common::thing::get_thing_factory().unwrap() = previous_factory;

    let foreign_template: std::sync::Arc<dyn gamelogic::thing_template::ThingTemplate> =
        std::sync::Arc::new(gamelogic::thing_template::DefaultThingTemplate::new(
            "ForeignSameId".into(),
        ));
    let mut foreign = gamelogic::object::Object::new_raw(
        foreign_template,
        OBJECT_ID,
        gamelogic::common::types::ObjectStatusMaskType::none(),
        None,
    );
    let foreign_drawable = std::sync::Arc::new(std::sync::RwLock::new(
        gamelogic::object::drawable::Drawable::new(
            FOREIGN_DRAWABLE_ID,
            OBJECT_ID,
            "ForeignSameIdDrawable".into(),
            gamelogic::object::drawable::DrawableType::Static,
        ),
    ));
    foreign.set_drawable(Some(std::sync::Arc::clone(&foreign_drawable)));
    let foreign = std::sync::Arc::new(std::sync::RwLock::new(foreign));
    gamelogic::object::registry::OBJECT_REGISTRY.register_object(OBJECT_ID, &foreign);

    let mut receiving_content = admitted.clone();
    let mut receiver = GameClient::new().unwrap();
    receiver
        .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
            &bytes,
            Some(&mut receiving_content),
            policy(),
            |id| {
                assert_eq!(id, OBJECT_ID);
                Ok((17, 34, 51))
            },
        )
        .unwrap();
    assert_eq!(
        receiver.get_drawable_for_object(OBJECT_ID),
        Some(drawable_id)
    );
    let restored = receiver.find_drawable_by_id(drawable_id).unwrap();
    let restored = restored.as_any().downcast_ref::<BasicDrawable>().unwrap();
    assert_eq!(restored.get_indicator_color(), Some((17, 34, 51)));

    let still_bound_foreign_drawable = foreign.read().unwrap().get_drawable().unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &still_bound_foreign_drawable,
        &foreign_drawable
    ));
    gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(OBJECT_ID);

    // The record and Drawable payload each encode the object ID. C++ rejects
    // an archive that tries to attach the payload to a different object.
    let mut mismatched = bytes.clone();
    let encoded_id = OBJECT_ID.to_le_bytes();
    let offsets: Vec<_> = mismatched
        .windows(encoded_id.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == encoded_id).then_some(offset))
        .collect();
    assert_eq!(offsets.len(), 2, "expected outer and inner object IDs");
    mismatched[offsets[1]..offsets[1] + encoded_id.len()]
        .copy_from_slice(&(OBJECT_ID + 1).to_le_bytes());
    let mut mismatched_content = admitted.clone();
    let mut mismatched_receiver = GameClient::new().unwrap();
    let error = mismatched_receiver
        .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
            &mismatched,
            Some(&mut mismatched_content),
            policy(),
            |_| Ok((17, 34, 51)),
        )
        .unwrap_err();
    assert!(error.contains("snapshot object does not match"));

    let mut no_resolver_content = admitted.clone();
    let mut no_resolver = GameClient::new().unwrap();
    let error = no_resolver
        .restore_from_xfer_bytes_with_receiving_content(&bytes, &mut no_resolver_content, policy())
        .unwrap_err();
    assert!(error.contains("receiving object resolver is required"));

    let mut no_catalog = GameClient::new().unwrap();
    let retained = no_catalog
        .register_drawable(Box::new(BasicDrawable::new(
            game_client_rust::drawable::DrawableId::INVALID,
        )))
        .unwrap();
    let error = no_catalog
        .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
            &bytes,
            None,
            policy(),
            |_| Ok((17, 34, 51)),
        )
        .unwrap_err();
    assert!(error.contains("receiving Drawable template catalog was not admitted"));
    assert!(no_catalog.find_drawable_by_id(retained).is_some());

    let empty_bytes = GameClient::new().unwrap().capture_xfer_bytes().unwrap();
    let mut empty_receiver = GameClient::new().unwrap();
    empty_receiver
        .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
            &empty_bytes,
            None,
            policy(),
            |_| panic!("an empty archive has no attached object"),
        )
        .unwrap();

    let mut missing_receiver_content = admitted;
    let mut missing_receiver = GameClient::new().unwrap();
    let error = missing_receiver
        .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
            &bytes,
            Some(&mut missing_receiver_content),
            policy(),
            |object_id| Err(format!("missing receiving Object {object_id}")),
        )
        .unwrap_err();
    assert!(error.contains("missing receiving Object"));
}
