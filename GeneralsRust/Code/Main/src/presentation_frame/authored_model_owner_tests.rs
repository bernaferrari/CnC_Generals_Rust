use super::*;
use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};

fn receiving_world(pristine: &str, damaged: &str) -> GameLogic {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object USA_Ranger\n Draw = W3DModelDraw ModuleTag_Draw\n DefaultConditionState\n Model = {pristine}\n End\n ConditionState = DAMAGED\n Model = {damaged}\n End\n End\nEnd\n"), "frame_models.ini").unwrap();
    let definition = parser.get_definition("USA_Ranger").unwrap();
    let mut template = ThingTemplate::new("USA_Ranger");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    template.set_health(100.0);
    let mut world = GameLogic::new();
    world.add_object(Object::new(template, ObjectId(42), Team::USA));
    world
}

#[test]
fn receiving_model_frame_selects_its_own_condition_table() {
    use crate::game_logic::host_enum_table_residual::MC_BIT_DAMAGED;
    let mut world = receiving_world("None", "ReceivingDamaged");
    let mut peer = receiving_world("PeerPristine", "None");
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert_eq!(
        peer_frame.objects[0].model_key.as_deref(),
        Some("PeerPristine")
    );
    let models = &peer_frame.objects[0].draw_models;
    assert!(std::sync::Arc::ptr_eq(
        models,
        &peer_frame.direct_host_drawables[0].object.draw_models,
    ));
    let input = UnitRenderInput::from_renderable(&peer_frame.objects[0]);
    assert!(std::sync::Arc::ptr_eq(models, &input.draw_models));
    let wire = bincode::config::legacy();
    let legacy = bincode::serde::encode_to_vec(models.to_vec(), wire).unwrap();
    assert_eq!(bincode::serde::encode_to_vec(models, wire).unwrap(), legacy);
    let (restored, consumed): (std::sync::Arc<[crate::assets::AuthoredDrawModel]>, _) =
        bincode::serde::decode_from_slice(&legacy, wire).unwrap();
    assert_eq!(consumed, legacy.len());
    assert_eq!(restored, *models);
    let pristine = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(pristine.objects[0].model_key.is_none());
    assert!(pristine.objects[0].draw_models.is_empty());
    let actor = world.host_object_mut(ObjectId(42)).unwrap();
    actor.body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged;
    actor.model_condition_bits = 1u128 << MC_BIT_DAMAGED;
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(pristine.objects[0].draw_models.is_empty());
    assert_eq!(
        frame.objects[0].model_key.as_deref(),
        Some("ReceivingDamaged")
    );
    assert_eq!(
        frame.objects[0].draw_models[0].model_key,
        "ReceivingDamaged"
    );
    assert_eq!(
        peer_frame.objects[0].model_key.as_deref(),
        Some("PeerPristine")
    );
}

#[test]
fn receiving_model_render_input_keeps_its_table_through_environment_and_disguise() {
    let mut world = receiving_world("None", "ReceivingDamaged");
    let frame = PresentationFrame::build_from_logic(&mut world, 0);
    let mut input =
        UnitRenderInput::from_renderable_with_environment(None, &frame.objects[0], false, false, 0);
    assert!(input.draw_models.is_empty());
    input.model_condition_bits =
        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_DAMAGED;
    // The renderer deliberately rebuilds damage bits from the frozen canonical
    // Body ordinal. A raw stale DAMAGED bit alone cannot override a pristine
    // Body; advance both facts to represent an actual damaged input.
    input.body_damage_state = 1;
    assert_ne!(
        input.model_condition_bits_with_combat_flags()
            & (1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_DAMAGED),
        0
    );
    input.resolve_draw_models_for_frozen_conditions(None);
    assert_eq!(input.model_key, "ReceivingDamaged");

    let donor = receiving_world("DisguiseActual", "None");
    let mut disguise = donor
        .host_object(ObjectId(42))
        .unwrap()
        .get_template()
        .clone();
    disguise.name = "ReceivingDisguise".into();
    world.templates.insert(disguise.name.clone(), disguise);
    let object = world.host_object_mut(ObjectId(42)).unwrap();
    object.status.disguised = true;
    object.disguise_as_template = Some("ReceivingDisguise".into());
    let disguised = PresentationFrame::build_from_logic(&mut world, 0);
    let inputs = disguised.unit_render_inputs();
    let visual = inputs
        .iter()
        .find(|input| input.id == ObjectId(42))
        .unwrap();
    assert_eq!(visual.template_name, "ReceivingDisguise");
    assert_eq!(visual.model_key, "DisguiseActual");
    assert_eq!(visual.draw_models[0].model_key, "DisguiseActual");
}

#[test]
fn rendering_uses_receiving_asset_catalogue_for_same_object_name() {
    use std::sync::{Arc, Mutex};

    fn manager(model: &str) -> Arc<Mutex<crate::assets::AssetManager>> {
        let mut manager = crate::assets::AssetManager::new().unwrap();
        manager.overlay_object_create_overrides(
            "SharedTree",
            "",
            &std::collections::HashMap::from([("Model".to_string(), model.to_string())]),
        );
        Arc::new(Mutex::new(manager))
    }

    let first = manager("FirstTreeMesh");
    let second = manager("SecondTreeMesh");
    let mut left = GameLogic::new();
    let mut right = GameLogic::new();
    left.bind_application_assets(Arc::clone(&first));
    right.bind_application_assets(Arc::clone(&second));
    // A bound world retains its provider independently of the compatibility
    // publication used by standalone callbacks.
    left.main_services.asset_manager.lock().unwrap().take();
    right.main_services.asset_manager.lock().unwrap().take();
    let mut template = ThingTemplate::new("SharedTree");
    template.add_kind_of(crate::game_logic::KindOf::Vehicle);
    template.set_health(100.0);
    left.add_object(Object::new(template.clone(), ObjectId(42), Team::USA));
    right.add_object(Object::new(template, ObjectId(42), Team::USA));
    let left_frame = PresentationFrame::try_build_from_logic(&mut left, 0).unwrap();
    let right_frame = PresentationFrame::try_build_from_logic(&mut right, 0).unwrap();
    let model = |frame: &PresentationFrame| {
        frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == ObjectId(42))
            .unwrap()
            .model_key
    };
    assert_eq!(model(&left_frame), "FirstTreeMesh");
    assert_eq!(model(&right_frame), "SecondTreeMesh");
    assert_eq!(model(&left_frame), "FirstTreeMesh");
}
