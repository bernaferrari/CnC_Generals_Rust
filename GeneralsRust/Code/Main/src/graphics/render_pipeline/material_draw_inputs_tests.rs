use super::*;
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use game_client::core::game_client::NativeMaterialObservation;
use game_client::drawable::drawable::StealthLook;

fn sync_entry(
    object_id: u32,
    host_epoch: u64,
    visual_template_name: &str,
    resident: bool,
    destroyed: bool,
    position: [f32; 3],
    orientation: f32,
) -> game_client::core::PresentationDrawableSync {
    game_client::core::PresentationDrawableSync {
        receiving_science_player: None,
        object_id,
        host_epoch,
        resident,
        visual_template_name: visual_template_name.to_string(),
        template_name: "UnderlyingTemplate".to_string(),
        position,
        orientation,
        float_yaw: 0.0,
        float_pitch: 0.0,
        destroyed,
        model_condition_bits: 0,
        body_damage_state: 0,
        kind_names: std::sync::Arc::new(Vec::new()),
        team_color: [1.0, 1.0, 1.0, 1.0],
        effectively_stealthed: false,
        scene_hidden_by_stealth: false,
        health_current: 0.0,
        health_max: 0.0,
        selected: false,
        veterancy_level: 0,
        under_construction: false,
        construction_percent: 0.0,
        sold: false,
        ammo_pip_total: 0,
        ammo_pip_full: 0,
        occupant_count: 0,
        max_garrison: 0,
        disabled: false,
        is_carbomb: false,
        bomb_type: 0,
        bomb_timer_seconds: 0,
        weapon_bonus_enthusiastic: false,
        show_healing: false,
        healing_icon_type: 0,
        garrisoned_ids: Vec::new(),
        emoticon_name: String::new(),
        emoticon_frames_left: 0,
        formation_id: 0,
        caption: String::new(),
        module_admission:
            game_client::core::game_client::PresentationModuleAdmission::Compatibility,
        draw_logic_frame: 0,
        draw_indicator_color: [0, 0, 0],
        draw_physics: Default::default(),
        draw_velocity_magnitude_squared: 0.0,
        draw_module_names: std::sync::Arc::new(Vec::new()),
        health_box_width: 0.0,
        health_box_z_offset: 0.0,
    }
}

fn world() -> (
    Rc<PresentationFrame>,
    game_client::core::GameClient,
    NativeMaterialObservation,
) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    let mut template = ThingTemplate::new("NativeMaterialTank");
    template.add_kind_of(KindOf::Vehicle);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("NativeMaterialTank", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut frame = PresentationFrame::build_from_logic(&mut logic, 0);
    frame.fow_shell_bypass = true;
    let frame = Rc::new(frame);
    assert!(
        frame
            .direct_host_drawables
            .iter()
            .any(|direct| direct.object.id.0 == id.0)
    );
    let mut client = game_client::core::GameClient::new().unwrap();
    client
        .sync_presentation_drawables([sync_entry(
            id.0,
            7,
            "NativeMaterialTank",
            true,
            false,
            [0.; 3],
            0.,
        )])
        .unwrap();
    let key = client
        .presentation_direct_drawable_state(7, id.0)
        .unwrap()
        .binding_key;
    let observation = NativeMaterialObservation {
        binding_key: key,
        effectively_dead: false,
        frenzy: false,
        look: StealthLook::VisibleDetected,
        is_disguised: false,
        is_mine: false,
        friendly_opacity: 0.5,
        pending_opacity: Some(1.),
    };
    (frame, client, observation)
}

fn sidecar(
    client: &game_client::core::GameClient,
    key: PresentationDirectDrawableBindingKey,
) -> HashMap<ObjectID, FrozenDirectDrawableShroudState> {
    let state = client
        .presentation_direct_drawable_state(key.host_epoch, key.object_id)
        .unwrap();
    HashMap::from([(
        ObjectID(key.object_id),
        FrozenDirectDrawableShroudState {
            host_epoch: key.host_epoch,
            object_id: ObjectID(key.object_id),
            drawable_id: key.drawable_id.0,
            binding_generation: key.binding_generation,
            scene_effectively_hidden: state.scene_effectively_hidden,
            fully_obscured: state.fully_obscured,
        },
    )])
}

#[test]
fn completed_receiver_material_reaches_main_inputs_and_refreshes_same_frozen_frame() {
    let (frame, mut client, observation) = world();
    client
        .prepare_presentation_materials(&[observation])
        .unwrap();
    let id = ObjectID(observation.binding_key.object_id);
    let mut inputs = vec![UnitRenderInput::from_renderable(
        &frame
            .direct_host_drawables
            .iter()
            .find(|direct| direct.object.id == id)
            .unwrap()
            .object,
    )];
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    assert!(
        frame
            .unit_render_inputs()
            .iter()
            .all(|input| input.second_material_pass_opacity == 0.)
    );
    let native = client
        .find_drawable_by_id_mut(observation.binding_key.drawable_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<game_client::drawable::BasicDrawable>()
        .unwrap();
    native.advance_material_draw(false, false);
    let old = FrozenMaterialDrawFrame::capture(&frame, &client, 7);
    let shroud = sidecar(&client, observation.binding_key);
    old.apply_to_inputs(&frame, &shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.8);
    client
        .find_drawable_by_id_mut(observation.binding_key.drawable_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<game_client::drawable::BasicDrawable>()
        .unwrap()
        .advance_material_draw(false, false);
    let current = FrozenMaterialDrawFrame::capture(&frame, &client, 7);
    current.apply_to_inputs(&frame, &shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.8 * 0.8);
    // Earlier completed render input never tracks the live owner after capture.
    old.apply_to_inputs(&frame, &shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.8);
    let replacement = Rc::new((*frame).clone());
    current.apply_to_inputs(&replacement, &shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    let mut stale = shroud.clone();
    stale.get_mut(&id).unwrap().binding_generation += 1;
    current.apply_to_inputs(&frame, &stale, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    let mut wrong_world = shroud.clone();
    wrong_world.get_mut(&id).unwrap().host_epoch += 1;
    current.apply_to_inputs(&frame, &wrong_world, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    let mut wrong_drawable = shroud.clone();
    wrong_drawable.get_mut(&id).unwrap().drawable_id += 1;
    current.apply_to_inputs(&frame, &wrong_drawable, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    let mut changed_shroud = shroud.clone();
    changed_shroud.get_mut(&id).unwrap().fully_obscured = !shroud[&id].fully_obscured;
    current.apply_to_inputs(&frame, &changed_shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
    inputs[0].drawable_shroud = Default::default();
    current.apply_to_inputs(&frame, &shroud, &mut inputs);
    assert_eq!(inputs[0].second_material_pass_opacity, 0.);
}
