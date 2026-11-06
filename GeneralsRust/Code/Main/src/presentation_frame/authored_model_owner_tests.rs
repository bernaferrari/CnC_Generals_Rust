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
fn receiving_model_frame_rejects_shadow_alias_and_selects_its_own_condition_table() {
    use crate::game_logic::host_enum_table_residual::MC_BIT_DAMAGED;
    let mut world = receiving_world("None", "ReceivingDamaged");
    let mut peer = receiving_world("PeerPristine", "None");
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert_eq!(
        peer_frame.objects[0].model_key.as_deref(),
        Some("PeerPristine")
    );
    let mut frame = PresentationFrame::build_from_logic(&mut world, 0);
    assert!(frame.objects[0].model_key.is_none());
    assert!(frame.objects[0].draw_models.is_empty());
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(16);
    shadow.sync_from_host(&world);
    let eid = shadow.entity_for_host(ObjectId(42)).unwrap();
    {
        let entity = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
        entity.model_key = "PoisonedShadowMesh".into();
        entity.model_condition_bits = 0;
    }
    frame.overlay_gameworld_shadow(&shadow);
    assert!(frame.objects[0].model_key.is_none());
    assert!(frame.objects[0].draw_models.is_empty());
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .model_condition_bits = 1u128 << MC_BIT_DAMAGED;
    frame.overlay_gameworld_shadow(&shadow);
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
fn shadow_renderable_uses_receiving_asset_catalogue_for_same_object_name() {
    use gamelogic::world::entities::{EntityStore, TemplateRef, Transform};
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
    let mut entities = EntityStore::new();
    let id = entities.spawn(TemplateRef::new("SharedTree"), None, Transform::default(), 100.0);
    let entity = entities.get(id).unwrap();
    let resolve = |owner: &Arc<Mutex<crate::assets::AssetManager>>| PresentationFrame::renderable_from_gameworld_entity(
        Some(owner.as_ref()), ObjectId(42), entity,
    ).model_key;
    assert_eq!(resolve(&first).as_deref(), Some("FirstTreeMesh"));
    assert_eq!(resolve(&second).as_deref(), Some("SecondTreeMesh"));
    assert_eq!(resolve(&first).as_deref(), Some("FirstTreeMesh"));

    let mut left = GameLogic::new();
    let mut right = GameLogic::new();
    left.bind_application_assets(Arc::clone(&first));
    right.bind_application_assets(Arc::clone(&second));
    let mut template = ThingTemplate::new("SharedTree");
    template.add_kind_of(crate::game_logic::KindOf::Vehicle);
    template.set_health(100.0);
    left.add_object(Object::new(template.clone(), ObjectId(42), Team::USA));
    right.add_object(Object::new(template, ObjectId(42), Team::USA));
    let left_frame = PresentationFrame::build_from_logic(&mut left, 0);
    let right_frame = PresentationFrame::build_from_logic(&mut right, 0);
    let model = |frame: &PresentationFrame| frame.unit_render_inputs()
        .into_iter().find(|input| input.id == ObjectId(42)).unwrap().model_key;
    assert_eq!(model(&left_frame), "FirstTreeMesh");
    assert_eq!(model(&right_frame), "SecondTreeMesh");
    assert_eq!(model(&left_frame), "FirstTreeMesh");
}
