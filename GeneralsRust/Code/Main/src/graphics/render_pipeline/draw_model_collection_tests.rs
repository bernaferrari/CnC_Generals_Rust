use super::*;

fn selected_modules() -> Vec<crate::assets::AuthoredDrawModel> {
    [7, 2, 7]
        .into_iter()
        .map(|module_index| crate::assets::AuthoredDrawModel {
            module_index,
            model_key: "avhummer".into(),
            transition_key: "construction_transition".into(),
            allow_to_finish_key: "finish_current_state".into(),
            animations: vec![crate::assets::AuthoredDrawAnimation {
                name: "AVHUMMER.AVHU_IDLE".into(),
                is_idle: true,
                distance_covered_token: Some("10.5".into()),
            }],
            subobject_visibility: vec![
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "barrel01".into(),
                    hidden: false,
                },
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "barrel02".into(),
                    hidden: true,
                },
            ],
            ..Default::default()
        })
        .collect()
}

#[test]
fn collection_moves_selected_modules_and_nested_payloads_in_source_order() {
    let mut selected = selected_modules();
    let expected = selected.clone();
    let vector = selected.as_ptr();
    let key = selected[0].model_key.as_ptr();
    let animations = selected[0].animations.as_ptr();
    let visibility = selected[0].subobject_visibility.as_ptr();
    let animation_name = selected[0].animations[0].name.as_ptr();
    let child_name = selected[0].subobject_visibility[0].name.as_ptr();

    let collected = selected_draw_models_for_collection(&mut selected, "ignored_fallback");
    assert!(
        selected.is_empty(),
        "owned Draw modules transfer once into collection"
    );
    assert_eq!(
        collected, expected,
        "source order, repeated identities and directives stay exact"
    );
    assert_eq!(
        collected.as_ptr(),
        vector,
        "reuse the already-owned vector allocation"
    );
    assert_eq!(collected[0].model_key.as_ptr(), key);
    assert_eq!(collected[0].animations.as_ptr(), animations);
    assert_eq!(collected[0].subobject_visibility.as_ptr(), visibility);
    assert_eq!(collected[0].animations[0].name.as_ptr(), animation_name);
    assert_eq!(
        collected[0].subobject_visibility[0].name.as_ptr(),
        child_name
    );
}

#[test]
fn collection_retains_empty_and_legacy_exact_model_key_fallbacks() {
    let mut empty = Vec::new();
    assert!(selected_draw_models_for_collection(&mut empty, "  ").is_empty());
    let key = " Art/W3D/ExactModel.w3d ";
    let collected = selected_draw_models_for_collection(&mut empty, key);
    assert_eq!(
        collected,
        vec![crate::assets::AuthoredDrawModel {
            module_index: 0,
            model_key: key.to_owned(),
            ..Default::default()
        }]
    );
    assert!(empty.is_empty());
}
