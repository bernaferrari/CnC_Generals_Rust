//! Actual receiving CPU constructors/receipts. No GPU/original archive parity claim.
//! Authored Model=NONE is a genuine objectless Drawable; distinct installed module tags
//! verify receiving template selection without inventing unavailable W3D asset files.
use game_client_rust::core::game_client::{
    ClientInputAuthority, GameClient, NativeAuxiliaryMutation, NativeAuxiliaryReceipt,
    NativeDrawCandidate, NativeDrawableBirthContext, ReceivingDrawablePolicy,
};
use game_client_rust::drawable::BasicDrawable;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module::StaticGameLodLevel;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_template::{
    AdmittedReceivingTemplate, ObjectIniFields, ReceivingTemplateKind, ThingTemplate,
};
use std::sync::Arc;

fn content(module_tag: &str) -> (ModuleFactory, Arc<AdmittedReceivingTemplate>) {
    let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
    let source = format!(
        "Draw = W3DModelDraw {module_tag}\nDefaultConditionState\nModel = NONE\nEnd\nEnd\nEnd"
    );
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
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    // This authored CPU fixture has no particle emitters. Explicitly complete
    // its own empty catalog admission rather than loading retail/global INI.
    client.particle_systems_mut().finish_definition_admission();
    client
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
                time_of_day: Some(game_client_rust::system::TimeOfDay::Afternoon),
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
fn installed_module_tag(client: &GameClient, receipt: &NativeAuxiliaryReceipt) -> String {
    let draw = client.find_drawable_by_id(receipt.drawable_id()).unwrap();
    assert!(draw.get_object_id().is_none());
    let basic = draw.as_any().downcast_ref::<BasicDrawable>().unwrap();
    assert_eq!(basic.get_draw_modules().len(), 1);
    basic.get_draw_modules()[0]
        .snapshot_module_identifier()
        .expect("actual receiving draw-module tag")
        .to_owned()
}

#[test]
fn equal_native_ids_keep_actual_modules_pose_and_retirement_in_the_receiving_owner() {
    let mut first = client();
    let mut peer = client();
    let first_receipt = create(&mut first, "ArrowA", 0.5);
    let peer_receipt = create(&mut peer, "ArrowB", 2.0);
    assert_eq!(first_receipt.drawable_id(), peer_receipt.drawable_id());
    assert_ne!(first_receipt, peer_receipt);
    assert_eq!(installed_module_tag(&first, &first_receipt), "ArrowA");
    assert_eq!(installed_module_tag(&peer, &peer_receipt), "ArrowB");
    {
        let (_, mut owner, ..) = first.split_render_bridge_drawables_and_ui_renderer();
        let mut transforms = Vec::new();
        owner
            .position_auxiliary_drawable(&first_receipt, [3.0, 5.0, 7.0], Some(0.7), |event| {
                if let NativeAuxiliaryMutation::TransformInstalled { transform, .. } = event {
                    transforms.push(transform);
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            transforms.len(),
            2,
            "setPosition callback precedes independent orientation callback"
        );
        assert_eq!(transforms[0].elements[0][2], 0.0);
        owner
            .set_auxiliary_scene_flags(&first_receipt, true, Some(44))
            .unwrap();
        let state = owner
            .observe_auxiliary_drawable(&first_receipt)
            .unwrap()
            .unwrap();
        assert_eq!(state.base.elements[0][3], 3.0);
        assert_eq!(state.base.elements[1][3], 5.0);
        assert_eq!(state.base.elements[2][3], 7.0);
        assert!(state.hidden);
        assert_eq!(state.shroud_object, 44);
        assert_eq!(state.scale, 0.5);
        // S * Rotate_Z(theta) * S is Rotate_Y(-theta), source Y/Z swap.
        assert!((state.base.elements[0][2] + 0.7_f32.sin()).abs() < 0.00001);
        assert!(
            owner
                .retire_auxiliary_drawable(&peer_receipt, |_| Ok(()))
                .is_err()
        );
        assert!(owner.auxiliary_is_live(&first_receipt));
    }
    peer.split_render_bridge_drawables_and_ui_renderer()
        .1
        .retire_auxiliary_drawable(&peer_receipt, |_| Ok(()))
        .unwrap();
    drop(peer);
    assert_eq!(installed_module_tag(&first, &first_receipt), "ArrowA");
    let (_, mut owner, ..) = first.split_render_bridge_drawables_and_ui_renderer();
    assert!(
        matches!(&owner.native_draw_candidates().unwrap()[0], NativeDrawCandidate::Aux(receipt) if receipt == &first_receipt)
    );
    owner
        .retire_auxiliary_drawable(&first_receipt, |_| Ok(()))
        .unwrap();
    assert!(!owner.auxiliary_is_live(&first_receipt));
}

fn saved_drawable_count(bytes: &[u8]) -> u16 {
    use game_engine::common::system::Xfer;
    let mut xfer =
        game_engine::common::system::xfer_load::XferLoad::new(std::io::Cursor::new(bytes), 3);
    let mut version = 0;
    xfer.xfer_version(&mut version, 3).unwrap();
    let mut frame = 0;
    xfer.xfer_unsigned_int(&mut frame).unwrap();
    let mut toc_version = 0;
    xfer.xfer_version(&mut toc_version, 1).unwrap();
    let mut count = 0;
    xfer.xfer_unsigned_int(&mut count).unwrap();
    for _ in 0..count {
        let mut name = String::new();
        let mut id = 0;
        xfer.xfer_ascii_string(&mut name).unwrap();
        xfer.xfer_unsigned_short(&mut id).unwrap();
    }
    let mut drawables = 0;
    xfer.xfer_unsigned_short(&mut drawables).unwrap();
    drawables
}
#[test]
fn rust_save_excludes_only_exact_claimed_auxiliary_receipts() {
    let mut first = client();
    let mut peer = client();
    let claimed = create(&mut first, "ArrowA", 0.5);
    let retained = create(&mut first, "ArrowA", 0.5);
    let foreign = create(&mut peer, "ArrowA", 0.5);
    assert_eq!(
        saved_drawable_count(
            &first
                .capture_xfer_bytes_with_owned_auxiliaries(&[])
                .unwrap()
        ),
        2
    );
    assert_eq!(
        saved_drawable_count(
            &first
                .capture_xfer_bytes_with_owned_auxiliaries(&[claimed.clone()])
                .unwrap()
        ),
        1
    );
    assert!(
        first
            .capture_xfer_bytes_with_owned_auxiliaries(&[foreign])
            .is_err()
    );
    let (_, owner, ..) = first.split_render_bridge_drawables_and_ui_renderer();
    assert!(owner.auxiliary_is_live(&claimed));
    assert!(owner.auxiliary_is_live(&retained));
}

#[test]
fn failed_actual_constructor_unlinks_only_its_receiving_node_before_resuming_panic() {
    let mut first = client();
    let mut peer = client();
    let peer_receipt = create(&mut peer, "PeerArrow", 0.5);
    let (mut factory, plan) = content("FailingArrow");
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        first
            .split_render_bridge_drawables_and_ui_renderer()
            .1
            .create_auxiliary_drawable(
                plan,
                &mut factory,
                0.5,
                ReceivingDrawablePolicy {
                    time_of_day: Some(game_client_rust::system::TimeOfDay::Afternoon),
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
                |event| {
                    if matches!(event, NativeAuxiliaryMutation::ConditionsMarked { .. }) {
                        panic!("constructor observer");
                    }
                    Ok(())
                },
            )
            .unwrap();
    }));
    assert!(failed.is_err());
    assert!(
        first
            .split_render_bridge_drawables_and_ui_renderer()
            .1
            .native_draw_candidates()
            .unwrap()
            .is_empty()
    );
    assert!(
        peer.split_render_bridge_drawables_and_ui_renderer()
            .1
            .auxiliary_is_live(&peer_receipt)
    );
}
