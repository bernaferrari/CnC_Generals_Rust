//! Direct renderer identity predicate with real same-ID peer Drawables.
use super::*;
use game_client::core::game_client::{
    ClientInputAuthority, GameClient, NativeAuxiliaryMutation, NativeAuxiliaryReceipt,
    NativeDrawCandidate, NativeDrawableBirthContext, ReceivingDrawablePolicy,
};
use game_client::drawable::BasicDrawable;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module::StaticGameLodLevel;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_template::{
    AdmittedReceivingTemplate, ObjectIniFields, ReceivingTemplateKind, ThingTemplate,
};
use std::sync::Arc;

fn content(model: &str) -> (ModuleFactory, Arc<AdmittedReceivingTemplate>) {
    let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
    let source =
        format!("Draw = W3DModelDraw Arrow\nDefaultConditionState\nModel = {model}\nEnd\nEnd\nEnd");
    let lines = source.lines().collect::<Vec<_>>();
    let fields = ObjectIniFields::capture_body(&lines, 0).0;
    let plan = ThingTemplate::new()
        .admit_named_receiving_module_source(
            "LockonCursor",
            &fields,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &mut factory,
        )
        .unwrap();
    (factory, Arc::new(plan))
}
fn client() -> GameClient {
    GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap()
}
fn create(client: &mut GameClient, model: &str, scale: f32) -> NativeAuxiliaryReceipt {
    let (mut factory, plan) = content(model);
    let mut phases = Vec::new();
    let receipt = client
        .split_render_bridge_drawables_and_ui_renderer()
        .1
        .create_auxiliary_drawable(
            plan,
            &mut factory,
            scale,
            ReceivingDrawablePolicy {
                time_of_day: Some(game_client::system::TimeOfDay::Afternoon),
                use_draw_module_lod: true,
                static_game_lod: StaticGameLodLevel::High,
                use_tree_sway: true,
            },
            NativeDrawableBirthContext {
                loading_map: false,
                in_load_game: false,
            },
            7,
            230,
            |mutation| {
                phases.push(match mutation {
                    NativeAuxiliaryMutation::Creating { .. } => "create",
                    NativeAuxiliaryMutation::ConditionsMarked { .. } => "conditions",
                    NativeAuxiliaryMutation::TransformInstalled { .. } => "transform",
                    NativeAuxiliaryMutation::Destroyed { .. } => "destroy",
                });
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(phases, ["create", "conditions"]);
    receipt
}

#[test]
fn renderer_identity_rejects_equal_id_and_generation_from_peer_client() {
    let mut a = client();
    let mut b = client();
    let receipt_a = create(&mut a, "ArrowA", 0.5);
    let receipt_b = create(&mut b, "ArrowB", 0.5);
    assert_eq!(receipt_a.drawable_id(), receipt_b.drawable_id());
    assert_eq!(receipt_a.generation(), receipt_b.generation());
    let world = Arc::new(());
    let identity_a =
        LiveDrawPlaybackIdentity::new_auxiliary(Arc::clone(&world), 7, &receipt_a).unwrap();
    let identity_b = LiveDrawPlaybackIdentity::new_auxiliary(world, 7, &receipt_b).unwrap();
    assert!(identity_a.matches_auxiliary_receipt(&receipt_a));
    assert!(identity_b.matches_auxiliary_receipt(&receipt_b));
    assert!(
        !identity_a.matches_auxiliary_receipt(&receipt_b),
        "direct render admission/Draw/retirement must reject a peer receipt even with equal numeric lifetime"
    );
    assert!(!identity_b.matches_auxiliary_receipt(&receipt_a));
    drop(b);
    assert!(identity_a.matches_auxiliary_receipt(&receipt_a));
    assert!(
        a.split_render_bridge_drawables_and_ui_renderer()
            .1
            .observe_auxiliary_drawable(&receipt_a)
            .unwrap()
            .is_some()
    );
}
