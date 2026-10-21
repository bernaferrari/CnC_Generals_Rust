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

#[test]
fn explicit_native_retirement_stops_local_ambient_after_dropping_modules() {
    if isolated("explicit_native_retirement_stops_local_ambient_after_dropping_modules") {
        return;
    }
    #[derive(Debug)]
    struct RetireProbe {
        bucket: usize,
        digit: usize,
        trace: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl DrawModule for RetireProbe {
        fn drawable_module_type_index(&self) -> usize {
            self.bucket
        }
    }
    impl Drop for RetireProbe {
        fn drop(&mut self) {
            let _ = self.trace.fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |value| Some(value * 10 + self.digit),
            );
        }
    }
    let trace = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut draw = BasicDrawable::new(DrawableId(709));
    custom(&mut draw, "RetirementAmbient");
    draw.initialize_native_constructor_template(Some("CtorTemplate".into()), Some(plan()));
    draw.finish_native_constructor_ambient(false, false);
    assert_eq!(
        draw.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "RetirementAmbient"
    );
    for (bucket, digit) in [(1, 3), (0, 1), (0, 2)] {
        draw.draw_modules.push(Box::new(RetireProbe {
            bucket,
            digit,
            trace: Arc::clone(&trace),
        }));
    }
    draw.finish_native_retirement();
    assert_eq!(
        trace.load(std::sync::atomic::Ordering::SeqCst),
        123,
        "C++ Draw bucket order precedes ClientUpdate despite mixed storage"
    );
    assert!(draw.ambient_sound_event.is_none());
    assert!(draw.draw_modules.is_empty());
    assert!(draw.custom_sound_ambient_dynamic_info.is_none());
    draw.finish_native_retirement();
    assert!(draw.ambient_sound_event.is_none());
}
