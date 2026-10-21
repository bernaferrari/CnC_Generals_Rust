//! Actual local Drawable ambient state, without claiming AudioManager delivery.
use super::*;
use game_engine::common::audio::audio_event_rts::AudioEventInfo;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_template::{
    AdmittedReceivingTemplate, ObjectIniFields, ReceivingTemplateKind,
};
use std::sync::Arc;

fn custom(draw: &mut BasicDrawable, name: &str) {
    let info = AudioEventInfo {
        audio_name: name.to_owned(),
        ..Default::default()
    };
    draw.custom_sound_ambient_dynamic_info = Some(DynamicAudioEventInfo::from_base_info(&info));
}
fn plan() -> Arc<AdmittedReceivingTemplate> {
    let mut catalog = ModuleFactory::new_receiving(NameKeyNamespace::new());
    let fields = ObjectIniFields::capture_body(&["End"], 0).0;
    Arc::new(
        game_engine::common::thing::ThingTemplate::new()
            .admit_named_receiving_module_source(
                "CtorTemplate",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                &mut catalog,
            )
            .unwrap(),
    )
}
fn isolated(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(module_path!(), name)
    {
        return true;
    }
    #[cfg(target_arch = "wasm32")]
    let _ = name;
    false
}
#[test]
fn native_constructor_template_is_inert_then_owned_gate_controls_local_ambient() {
    if isolated("native_constructor_template_is_inert_then_owned_gate_controls_local_ambient") {
        return;
    }
    for (loading_map, in_load_game, should_start) in [
        (false, false, true),
        (true, false, false),
        (false, true, false),
        (true, true, false),
    ] {
        let mut draw = BasicDrawable::new(DrawableId(701));
        assert!(draw.ambient_sound_enabled && draw.ambient_sound_enabled_from_script);
        custom(&mut draw, "ConstructorAmbient");
        assert!(
            draw.initialize_native_constructor_template(Some("CtorTemplate".into()), Some(plan()))
        );
        assert!(
            draw.ambient_sound_event.is_none(),
            "definition initialization cannot emit constructor audio"
        );
        draw.finish_native_constructor_ambient(loading_map, in_load_game);
        assert_eq!(draw.ambient_sound_is_active(), should_start);
        if should_start {
            assert_eq!(
                draw.ambient_sound_event.as_ref().unwrap().get_event_name(),
                "ConstructorAmbient"
            );
        }
    }
}
#[test]
fn object_binding_does_not_bypass_suppressed_constructor_ambient() {
    if isolated("object_binding_does_not_bypass_suppressed_constructor_ambient") {
        return;
    }
    for (loading_map, in_load_game) in [(true, false), (false, true), (true, true)] {
        let mut draw = BasicDrawable::new(DrawableId(702));
        custom(&mut draw, "SuppressedConstructorAmbient");
        draw.initialize_native_constructor_template(Some("CtorTemplate".into()), Some(plan()));
        draw.finish_native_constructor_ambient(loading_map, in_load_game);
        assert!(draw.ambient_sound_event.is_none());
        draw.set_object_id(Some(42));
        assert_eq!(draw.object_id(), Some(42));
        assert!(
            draw.ambient_sound_event.is_none(),
            "friend_bindToObject cannot start a suppressed constructor event"
        );
    }
}
#[test]
fn retained_template_observation_preserves_same_arc_and_restarts_replacement() {
    if isolated("retained_template_observation_preserves_same_arc_and_restarts_replacement") {
        return;
    }
    let mut draw = BasicDrawable::new(DrawableId(703));
    let first = plan();
    let replacement = plan();
    assert!(!Arc::ptr_eq(&first, &replacement));
    custom(&mut draw, "FirstAmbient");
    draw.set_template_name_with_receiving(Some("CtorTemplate".into()), Some(Arc::clone(&first)));
    assert_eq!(
        draw.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "FirstAmbient"
    );
    custom(&mut draw, "ReplacementAmbient");
    draw.set_template_name_with_receiving(Some("CtorTemplate".into()), Some(first));
    assert_eq!(
        draw.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "FirstAmbient",
        "same Arc observation does not restart"
    );
    draw.set_template_name_with_receiving(Some("CtorTemplate".into()), Some(replacement));
    assert_eq!(
        draw.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "ReplacementAmbient"
    );
}
