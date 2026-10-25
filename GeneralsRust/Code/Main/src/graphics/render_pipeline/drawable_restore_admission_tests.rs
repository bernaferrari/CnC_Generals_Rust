use super::*;

fn frame_with_model(model: &str) -> crate::presentation_frame::PresentationFrame {
    frame_with_model_visibility(model, false)
}
fn frame_with_model_visibility(
    model: &str,
    contained: bool,
) -> crate::presentation_frame::PresentationFrame {
    use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        &format!("Object RestoreReceiver\n Draw = W3DModelDraw ModuleTag_Draw\n DefaultConditionState\n Model = {model}\n End\n End\nEnd\n"),
        "restore_receiver.ini",
    ).unwrap();
    let definition = parser.get_definition("RestoreReceiver").unwrap();
    let mut template = ThingTemplate::new("RestoreReceiver");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    let mut world = GameLogic::new();
    let mut object = Object::new(template, ObjectId(42), Team::USA);
    if contained {
        object.contained_by = Some(ObjectId(99));
    }
    world.add_object(object);
    if contained {
        world.add_object(Object::new(
            ThingTemplate::new("Container"),
            ObjectId(99),
            Team::USA,
        ));
    }
    crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0)
}

fn saved_receiver(
    frame: &crate::presentation_frame::PresentationFrame,
) -> ClientDrawableStateSnapshot {
    let units = frame.unit_render_inputs();
    let unit = units.iter().find(|unit| unit.id.0 == 42).unwrap();
    let draw = &unit.draw_models[0];
    ClientDrawableStateSnapshot {
        object_id: 42,
        draw_module_index: draw.module_index,
        source_template_name: unit.template_name.clone(),
        model_key: draw.model_key.clone(),
        selected_condition_state_index: draw.selected_condition_state_index,
        last_seen_weapon_discharge_sequence: 17,
        ..Default::default()
    }
}

#[test]
fn actual_collection_admits_detached_restore_once_against_receiving_frame() {
    let frame = frame_with_model("ReceivingMesh");
    let saved = saved_receiver(&frame);
    let key = (saved.object_id, saved.draw_module_index);
    let mut pending = Some(ClientDrawableWorldSnapshot {
        drawables: vec![saved.clone()],
    });
    let mut imports = HashMap::new();
    prepare_client_drawable_restore_for_collection(&mut pending, &mut imports, &frame);
    assert!(pending.is_none());
    assert_eq!(imports.remove(&key), Some(saved));
    // A second collect cannot replay the same payload after model resolution
    // consumed its candidate, including when that resolution found no asset.
    prepare_client_drawable_restore_for_collection(&mut pending, &mut imports, &frame);
    assert!(imports.is_empty());
}

#[test]
fn mismatched_receiving_visual_discards_restore_without_future_replay() {
    let source = frame_with_model("SavedMesh");
    let receiving = frame_with_model("ReplacementMesh");
    let mut pending = Some(ClientDrawableWorldSnapshot {
        drawables: vec![saved_receiver(&source)],
    });
    let mut imports = HashMap::new();
    prepare_client_drawable_restore_for_collection(&mut pending, &mut imports, &receiving);
    assert!(pending.is_none());
    assert!(
        imports.is_empty(),
        "equal ObjectId cannot import a different model's state"
    );
    prepare_client_drawable_restore_for_collection(&mut pending, &mut imports, &source);
    assert!(
        imports.is_empty(),
        "later compatible art must not resurrect rejected saved playback"
    );
}

#[test]
fn contained_resident_restore_identity_survives_visibility_filtered_collection_inputs() {
    let source = frame_with_model("ReceivingMesh");
    let saved = saved_receiver(&source);
    let hidden = frame_with_model_visibility("ReceivingMesh", true);
    assert!(
        hidden
            .unit_render_inputs()
            .iter()
            .all(|unit| unit.id.0 != 42)
    );
    assert!(
        hidden
            .native_resident_drawable_inputs()
            .iter()
            .any(|unit| unit.id.0 == 42)
    );
    let mut pending = Some(ClientDrawableWorldSnapshot {
        drawables: vec![saved.clone()],
    });
    let mut imports = HashMap::new();
    prepare_client_drawable_restore_for_collection(&mut pending, &mut imports, &hidden);
    assert_eq!(
        imports.remove(&(saved.object_id, saved.draw_module_index)),
        Some(saved)
    );
    assert!(pending.is_none());
}
