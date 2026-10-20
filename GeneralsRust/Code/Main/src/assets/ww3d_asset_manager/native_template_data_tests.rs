//! W3DModuleFactory data registration must precede immutable Object admission.
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_factory::ThingFactory;

#[test]
fn actual_source_parser_admits_typed_default_draw_before_native_client_assembly() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_source_parser_admits_typed_default_draw_before_native_client_assembly",
        || {
            // Exact source shape of the retail model-less ambient Drawable.
            // No handwritten typed ModuleData is injected into this template.
            let source = "Object AmbientTypedAdmission\n KindOf = IMMOBILE\n Draw = W3DDefaultDraw ModuleTag_01\n End\nEnd\n";
            let mut untyped = ThingFactory::new();
            assert_eq!(untyped.load_ini_text(source), 1);
            let old = untyped
                .find_template("AmbientTypedAdmission", false)
                .unwrap();
            let mut client = game_client::core::game_client::GameClient::new().unwrap();
            let error = client.create_drawable_from_template(&old).unwrap_err();
            assert!(error.to_string().contains("expected typed data"), "{error}");

            let mut catalog = ModuleFactory::new();
            gamelogic::contain_module_overrides::register_native_module_overrides(&mut catalog)
                .unwrap();
            let mut receiving = ThingFactory::new();
            assert_eq!(
                receiving.load_ini_text_with_module_factory(source, &mut catalog),
                1
            );
            let template = receiving
                .find_template("AmbientTypedAdmission", false)
                .unwrap();
            let entries: Vec<_> = template.get_draw_module_info().iter().collect();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].module_tag.as_str(), "ModuleTag_01");
            assert!(
                entries[0]
                    .data
                    .as_any()
                    .is::<gamelogic::object::draw::W3DDefaultDrawModuleData>()
            );
            // Constructor catalog lifetime is not mutable runtime ownership.
            drop(catalog);
            let drawable = client.create_drawable_from_template(&template).unwrap();
            assert!(client.find_drawable_by_id(drawable).is_some());
            assert!(
                !old.get_draw_module_info()
                    .iter()
                    .next()
                    .unwrap()
                    .data
                    .as_any()
                    .is::<gamelogic::object::draw::W3DDefaultDrawModuleData>(),
                "same-name peer must not be repaired or mutated"
            );
        },
    );
}
