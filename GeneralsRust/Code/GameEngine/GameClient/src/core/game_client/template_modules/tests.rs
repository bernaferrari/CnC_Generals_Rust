use super::*;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

type NativeCase = (&'static str, Arc<dyn ModuleData>, fn(&dyn Module) -> bool);

fn draw_cases() -> Vec<NativeCase> {
    macro_rules! cases {
        ($(($module:ident, $data:ident)),+ $(,)?) => {
            vec![$((stringify!($module), Arc::new($data::new()) as Arc<dyn ModuleData>,
                (|module: &dyn Module| module.as_any().is::<$module>()) as fn(&dyn Module) -> bool)),+]
        };
    }
    cases!(
        (W3DModelDraw, W3DModelDrawModuleData),
        (W3DDefaultDraw, W3DDefaultDrawModuleData),
        (W3DDependencyModelDraw, W3DDependencyModelDrawModuleData),
        (W3DTankDraw, W3DTankDrawModuleData),
        (W3DOverlordTankDraw, W3DOverlordTankDrawModuleData),
        (W3DOverlordAircraftDraw, W3DOverlordAircraftDrawModuleData),
        (W3DOverlordTruckDraw, W3DOverlordTruckDrawModuleData),
        (W3DPoliceCarDraw, W3DPoliceCarDrawModuleData),
        (W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData),
        (W3DRopeDraw, W3DRopeDrawModuleData),
        (W3DScienceModelDraw, W3DScienceModelDrawModuleData),
        (W3DSupplyDraw, W3DSupplyDrawModuleData),
        (W3DTankTruckDraw, W3DTankTruckDrawModuleData),
        (W3DTracerDraw, W3DTracerDrawModuleData),
        (W3DTreeDraw, W3DTreeDrawModuleData),
        (W3DPropDraw, W3DPropDrawModuleData),
        (W3DTruckDraw, W3DTruckDrawModuleData),
        (W3DLaserDraw, W3DLaserDrawModuleData),
        (W3DDebrisDraw, W3DDebrisDrawModuleData),
    )
}

fn client_cases() -> Vec<NativeCase> {
    vec![
        (
            "LaserUpdate",
            Arc::new(LaserUpdateModuleData::default()),
            |module| module.as_any().is::<LaserUpdateModule>(),
        ),
        (
            "BeaconClientUpdate",
            Arc::new(BeaconClientUpdateModuleData::default()),
            |module| module.as_any().is::<BeaconClientUpdateModule>(),
        ),
        (
            "SwayClientUpdate",
            Arc::new(BaseModuleData::new()),
            |module| module.as_any().is::<SwayClientUpdateModule>(),
        ),
        (
            "AnimatedParticleSysBoneClientUpdate",
            Arc::new(BaseModuleData::new()),
            |module| {
                module
                    .as_any()
                    .is::<AnimatedParticleSysBoneClientUpdateModule>()
            },
        ),
    ]
}

fn complete_template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("AllNativeModules".into());
    for (index, (name, data, _)) in draw_cases().into_iter().enumerate() {
        template.add_draw_module_info(
            name.into(),
            format!("DrawTag{index}").into(),
            data,
            ModuleInterfaceType::DRAW,
        );
    }
    for (index, (name, data, _)) in client_cases().into_iter().enumerate() {
        template.add_client_update_module_info(
            name.into(),
            format!("ClientTag{index}").into(),
            data,
            ModuleInterfaceType::CLIENT_UPDATE,
        );
    }
    template
}

fn basic(client: &mut GameClient, id: DrawableId) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

#[test]
fn real_factory_constructs_all_nineteen_draw_and_four_client_update_classes_in_order() {
    let mut a = GameClient::new().unwrap();
    let mut b = GameClient::new().unwrap();
    let mut a_keys = NameKeyNamespace::new();
    let mut b_keys = NameKeyNamespace::new();
    b_keys.name_to_key("ForeignPrefix");
    let template = complete_template();
    let a_id = a.create_drawable_from_template(&template).unwrap();
    let b_id = b.create_drawable_from_template(&template).unwrap();
    assert_eq!(a_id, b_id);
    let a_modules = basic(&mut a, a_id).get_draw_modules_mut();
    let b_modules = basic(&mut b, b_id).get_draw_modules_mut();
    assert_eq!(a_modules.len(), 23);
    assert_eq!(b_modules.len(), 23);
    for (index, ((name, _, is_expected), (a_module, b_module))) in draw_cases()
        .into_iter()
        .chain(client_cases())
        .zip(a_modules.iter_mut().zip(b_modules))
        .enumerate()
    {
        let tag = if index < 19 {
            format!("DrawTag{index}")
        } else {
            format!("ClientTag{}", index - 19)
        };
        assert_eq!(
            a_module.snapshot_module_identifier(),
            Some(tag.as_str()),
            "{name}"
        );
        assert_eq!(
            b_module.snapshot_module_identifier(),
            Some(tag.as_str()),
            "{name}"
        );
        assert_eq!(
            a_module.drawable_module_type_index(),
            usize::from(index >= 19),
            "{name}"
        );
        let a_logic = a_module.logic_module_mut().unwrap();
        let b_logic = b_module.logic_module_mut().unwrap();
        assert!(is_expected(a_logic), "actual module class {name}");
        assert!(is_expected(b_logic), "peer actual module class {name}");
        {
            let a_key = a_logic.get_module_name_key_with_namespace(&mut a_keys);
            let b_key = b_logic.get_module_name_key_with_namespace(&mut b_keys);
            assert_eq!(a_keys.key_to_name(a_key).as_deref(), Some(name));
            assert_eq!(b_keys.key_to_name(b_key).as_deref(), Some(name));
            assert_ne!(
                a_key, b_key,
                "{name} must query the supplied owner namespace"
            );
        }
        if index >= 20 {
            let admitted = template
                .get_client_update_module_info()
                .get_nth_data(index - 19)
                .unwrap();
            let original = Arc::as_ptr(admitted) as *const ();
            assert_eq!(
                a_logic.get_module_data() as *const dyn ModuleData as *const (),
                original,
                "{name} must share admitted immutable data"
            );
            assert_eq!(
                b_logic.get_module_data() as *const dyn ModuleData as *const (),
                original,
                "peer {name} must share the same admitted immutable data"
            );
        }
        assert_ne!(
            a_logic as *const dyn Module as *const (), b_logic as *const dyn Module as *const (),
            "{name} must have separate receiving allocations"
        );
        if let Some(laser) = a_logic.as_any_mut().downcast_mut::<LaserUpdateModule>() {
            assert!(laser.uses_owned_services());
        }
    }
    assert_eq!(a.drawables.owned_laser_order, [a_id]);
    assert_eq!(b.drawables.owned_laser_order, [b_id]);
}

#[test]
fn actual_template_modules_keep_their_native_xfer_payloads() {
    let template = complete_template();
    let mut a = GameClient::new().unwrap();
    let mut b = GameClient::new().unwrap();
    let a_id = a.create_drawable_from_template(&template).unwrap();
    let b_id = b.create_drawable_from_template(&template).unwrap();
    let a_modules = basic(&mut a, a_id).get_draw_modules_mut();
    let b_modules = basic(&mut b, b_id).get_draw_modules_mut();
    assert_eq!(
        a_modules.len(),
        23,
        "all nineteen Draw and four ClientUpdate payloads must exist"
    );
    assert_eq!(
        b_modules.len(),
        23,
        "restore receiver must contain all twenty-three native modules"
    );
    let expected = draw_cases()
        .into_iter()
        .chain(client_cases())
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 23, "independent authored class inventory");
    for (index, ((name, _, is_expected), (a_module, b_module))) in expected
        .into_iter()
        .zip(a_modules.iter_mut().zip(b_modules))
        .enumerate()
    {
        let tag = if index < 19 {
            format!("DrawTag{index}")
        } else {
            format!("ClientTag{}", index - 19)
        };
        for module in [&mut *a_module, &mut *b_module] {
            assert_eq!(
                module.snapshot_module_identifier(),
                Some(tag.as_str()),
                "{name} tag"
            );
            assert_eq!(
                module.drawable_module_type_index(),
                usize::from(index >= 19),
                "{name} bucket"
            );
            assert!(
                is_expected(module.logic_module_mut().expect("actual native module")),
                "{name} class"
            );
        }
        let mut saved = Cursor::new(Vec::new());
        a_module
            .xfer_snapshot(&mut XferSave::new(&mut saved, 1))
            .unwrap();
        let bytes = saved.into_inner();
        // Every listed runtime module writes its own Xfer version (or the
        // inherited DrawModule/DrawableModule version chain); none is a
        // legitimate empty Snapshot payload. CRC emptiness is a separate API.
        assert!(
            !bytes.is_empty(),
            "{name} must write its native Xfer version"
        );
        let mut input = Cursor::new(bytes.clone());
        b_module
            .xfer_snapshot(&mut XferLoad::new(&mut input, 1))
            .unwrap();
        assert_eq!(
            input.position() as usize,
            bytes.len(),
            "exact native module payload"
        );
        let mut resaved = Cursor::new(Vec::new());
        b_module
            .xfer_snapshot(&mut XferSave::new(&mut resaved, 1))
            .unwrap();
        assert_eq!(
            resaved.into_inner(),
            bytes,
            "{}",
            a_module.snapshot_module_identifier().unwrap()
        );
    }
}

fn baseline_template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("PreservedReceiver".into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "Baseline".into(),
        Arc::new(W3DModelDrawModuleData::new()),
        ModuleInterfaceType::DRAW,
    );
    template
}

#[test]
fn every_mistyped_class_is_rejected_without_receiver_identity_or_binding_changes() {
    let mut receiver = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let resident = receiver
        .create_drawable_from_template(&baseline_template())
        .unwrap();
    let peer_resident = peer
        .create_drawable_from_template(&baseline_template())
        .unwrap();
    receiver.bind_drawable_to_object(resident, 42).unwrap();
    peer.bind_drawable_to_object(peer_resident, 42).unwrap();
    receiver.add_text_bearing_drawable(resident);
    let original =
        receiver.find_drawable_by_id(resident).unwrap() as *const dyn Drawable as *const ();
    let next = receiver.drawables.next_drawable_id;
    let peer_next = peer.drawables.next_drawable_id;
    for (is_draw, (name, _, _)) in draw_cases()
        .into_iter()
        .map(|case| (true, case))
        .chain(client_cases().into_iter().map(|case| (false, case)))
    {
        let mut malformed = baseline_template();
        if is_draw {
            malformed.add_draw_module_info(
                name.into(),
                "Invalid".into(),
                Arc::new(BaseModuleData::new()),
                ModuleInterfaceType::DRAW,
            );
        } else {
            malformed.add_client_update_module_info(
                name.into(),
                "Invalid".into(),
                Arc::new(W3DDefaultDrawModuleData::new()),
                ModuleInterfaceType::CLIENT_UPDATE,
            );
        }
        let error = receiver
            .create_drawable_from_template(&malformed)
            .unwrap_err()
            .to_string();
        assert!(error.contains(name), "{error}");
        assert!(error.contains("Invalid"), "tag diagnostic: {error}");
        assert_eq!(receiver.drawables.next_drawable_id, next);
        assert_eq!(peer.drawables.next_drawable_id, peer_next);
        assert_eq!(receiver.get_drawable_ids(), [resident]);
        assert_eq!(peer.get_drawable_ids(), [peer_resident]);
        assert_eq!(receiver.get_drawable_for_object(42), Some(resident));
        assert_eq!(peer.get_drawable_for_object(42), Some(peer_resident));
        assert_eq!(receiver.drawables.text_bearing_drawables, [resident]);
        assert_eq!(
            receiver.find_drawable_by_id(resident).unwrap() as *const dyn Drawable as *const (),
            original
        );
    }
    assert_eq!(
        receiver
            .create_drawable_from_template(&baseline_template())
            .unwrap(),
        next
    );
}

#[test]
fn unknown_missing_duplicate_and_wrong_bucket_declarations_fail_before_construction() {
    let mut receiver = GameClient::new().unwrap();
    let next = receiver.drawables.next_drawable_id;
    for (name, tag, mask) in [
        ("NoSuchDrawClass", "Unknown", ModuleInterfaceType::DRAW),
        ("", "MissingClass", ModuleInterfaceType::DRAW),
        ("W3DDefaultDraw", "", ModuleInterfaceType::DRAW),
        ("W3DDefaultDraw", "Baseline", ModuleInterfaceType::DRAW),
        (
            "W3DDefaultDraw",
            "WrongBucket",
            ModuleInterfaceType::CLIENT_UPDATE,
        ),
    ] {
        let mut template = baseline_template();
        template.add_draw_module_info(
            name.into(),
            tag.into(),
            Arc::new(W3DDefaultDrawModuleData::new()),
            mask,
        );
        assert!(
            receiver.create_drawable_from_template(&template).is_err(),
            "{name}/{tag}"
        );
        assert_eq!(receiver.drawables.next_drawable_id, next);
        assert!(receiver.get_drawable_ids().is_empty());
    }
    let mut template = baseline_template();
    template.add_client_update_module_info(
        "BeaconClientUpdate".into(),
        "Beacon".into(),
        Arc::new(BeaconClientUpdateModuleData::default()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    template.add_client_update_module_info(
        "NoSuchClientUpdate".into(),
        "UnknownClient".into(),
        Arc::new(BaseModuleData::new()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let before = NameKeyGenerator::name_to_key("NativeFactoryPreflightBefore");
    assert!(receiver.create_drawable_from_template(&template).is_err());
    let after = NameKeyGenerator::name_to_key("NativeFactoryPreflightAfter");
    assert_eq!(
        after,
        before + 1,
        "preflight must reject before even constructing preceding valid callbacks"
    );
    assert_eq!(receiver.drawables.next_drawable_id, next);
}

#[derive(Debug, Default)]
struct BeaconDataView(BeaconClientUpdateModuleData);

impl ModuleData for BeaconDataView {
    fn set_module_tag_name_key(&mut self, key: u32) {
        self.0.module_tag_name_key = key;
    }
    fn get_module_tag_name_key(&self) -> u32 {
        self.0.module_tag_name_key
    }
    fn as_any(&self) -> &dyn std::any::Any {
        &self.0
    }
}

impl game_engine::common::system::Snapshotable for BeaconDataView {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        game_engine::common::system::Snapshotable::crc(&self.0, xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        game_engine::common::system::Snapshotable::xfer(&mut self.0, xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        game_engine::common::system::Snapshotable::load_post_process(&mut self.0)
    }
}

#[test]
fn wrapped_beacon_view_is_rejected_during_preflight_before_any_receiving_mutation() {
    let mut receiver = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let resident = receiver
        .create_drawable_from_template(&baseline_template())
        .unwrap();
    let peer_resident = peer
        .create_drawable_from_template(&baseline_template())
        .unwrap();
    receiver.bind_drawable_to_object(resident, 42).unwrap();
    peer.bind_drawable_to_object(peer_resident, 42).unwrap();
    let next = receiver.drawables.next_drawable_id;
    let peer_next = peer.drawables.next_drawable_id;
    let wrapped: Arc<dyn ModuleData> = Arc::new(BeaconDataView::default());
    assert!(
        wrapped.as_any().is::<BeaconClientUpdateModuleData>(),
        "fixture deliberately exposes a valid inner view"
    );
    let mut malformed = baseline_template();
    malformed.add_client_update_module_info(
        "BeaconClientUpdate".into(),
        "WrappedBeacon".into(),
        Arc::clone(&wrapped),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let count = Arc::strong_count(&wrapped);
    let before = NameKeyGenerator::name_to_key("WrappedBeaconPreflightBefore");
    let error = receiver
        .create_drawable_from_template(&malformed)
        .unwrap_err()
        .to_string();
    let after = NameKeyGenerator::name_to_key("WrappedBeaconPreflightAfter");
    assert!(error.contains("WrappedBeacon"), "{error}");
    assert!(
        error.contains("allocation"),
        "must reject the actual Arc type: {error}"
    );
    assert_eq!(
        after,
        before + 1,
        "class-key allocation occurred before rejecting data ownership"
    );
    assert_eq!(
        Arc::strong_count(&wrapped),
        count,
        "failed preflight retained the data Arc"
    );
    assert_eq!(receiver.drawables.next_drawable_id, next);
    assert_eq!(peer.drawables.next_drawable_id, peer_next);
    assert_eq!(receiver.get_drawable_ids(), [resident]);
    assert_eq!(peer.get_drawable_ids(), [peer_resident]);
    assert_eq!(receiver.get_drawable_for_object(42), Some(resident));
    assert_eq!(peer.get_drawable_for_object(42), Some(peer_resident));
}
