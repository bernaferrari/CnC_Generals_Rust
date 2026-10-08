//! Installed named-state metadata tests, not loaded-asset or GPU render proof.

use super::*;

fn state_data(offset: Option<Coord3D>, attach: bool) -> W3DModelDrawModuleData {
    let mut data = model_data(true);
    if !attach {
        data.attach_to_drawable_bone = AsciiString::new();
    }
    let state = &mut data.condition_states[0];
    state.model_name = AsciiString::from("NamedStateMetadata");
    let key = NameKeyGenerator::name_to_key("attach");
    if let Some(offset) = offset {
        state.pristine_bones.get_mut(&key).unwrap().transform = Matrix3D::from_translation(offset);
    } else {
        state.pristine_bones.remove(&key);
    }
    data
}

#[test]
fn installed_named_state_attachment_queries_current_and_siblings_in_order() {
    const CHILD: &str = "GENERALS_ACTIVE_RENDER_ATTACH_OWNER_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!().strip_prefix(prefix).unwrap();
    let test_name =
        format!("{module}::installed_named_state_attachment_queries_current_and_siblings_in_order");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&test_name, CHILD)
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    let before = Coord3D::new(1.0, 2.0, 3.0);
    let current = Coord3D::new(4.0, 5.0, 6.0);
    let after = Coord3D::new(7.0, 8.0, 9.0);
    // CPP Drawable.cpp:747-770 and W3DModelDraw.cpp:1125-1151:
    // maxBones=1 selects the first success; a miss caches origin.
    for (case, (offsets, expected)) in [
        ([Some(before), Some(current), Some(after)], before),
        ([None, Some(current), Some(after)], current),
        ([None, None, Some(after)], after),
        ([None, None, None], Coord3D::origin()),
    ]
    .into_iter()
    .enumerate()
    {
        let id = 0xA77A_1100 + case as u32;
        let mut drawable =
            Drawable::new(id, id, format!("ActiveAttach{case}"), DrawableType::Static);
        let mut entries = Vec::new();
        for (index, offset) in offsets.into_iter().enumerate() {
            let data = state_data(offset, index == 1);
            let mut model = W3DModelDraw::new(data.clone());
            model.owner_id = Some(id);
            model.cur_state = Some(ActiveModelState::Condition(0));
            entries.push(drawable.add_module(
                ModuleInterfaceType::DRAW,
                "W3DModelDraw".into(),
                format!("ActiveAttach{case}_{index}").into(),
                Arc::new(data),
                Box::new(model),
            ));
        }
        let drawable = Arc::new(RwLock::new(drawable));
        let mut owner = crate::object::Object::new_test(id, 100.0);
        owner.set_drawable(Some(Arc::clone(&drawable)));
        let owner = Arc::new(RwLock::new(owner));
        drawable.write().unwrap().bind_object_ref(&owner);
        OBJECT_REGISTRY.register_object(id, &owner);
        let _registered = Registered([id]);

        eprintln!("render child: before installed named-state cold attachment, case {case}");
        crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
        entries[1].with_module(|module| {
            let model = (module as &mut dyn std::any::Any)
                .downcast_mut::<W3DModelDraw>()
                .unwrap();
            assert_eq!(*model.attach_offset_cache.lock().unwrap(), Some(expected));
        });
        // A hot cache retains its original offset even if the source bone changes.
        for entry in &entries {
            entry.with_module(|module| {
                let model = (module as &mut dyn std::any::Any)
                    .downcast_mut::<W3DModelDraw>()
                    .unwrap();
                if let Some(bone) = model.data.condition_states[0]
                    .pristine_bones
                    .get_mut(&NameKeyGenerator::name_to_key("attach"))
                {
                    bone.transform = Matrix3D::from_translation(Coord3D::new(21.0, 22.0, 23.0));
                }
            });
        }
        crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
        entries[1].with_module(|module| {
            let model = (module as &mut dyn std::any::Any)
                .downcast_mut::<W3DModelDraw>()
                .unwrap();
            assert_eq!(*model.attach_offset_cache.lock().unwrap(), Some(expected));
        });
    }
}

#[test]
fn same_id_render_callbacks_use_their_exact_driving_drawable() {
    const CHILD: &str = "GENERALS_SAME_ID_RENDER_ATTACH_OWNER_CHILD";
    let module = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        &format!("{module}::same_id_render_callbacks_use_their_exact_driving_drawable"),
        CHILD,
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0xA77A_1104;
    let first_offset = Coord3D::new(12.0, 13.0, 14.0);
    let second_offset = Coord3D::new(42.0, 43.0, 44.0);
    let (first_owner, first, first_entry) = install_owner(id, state_data(Some(first_offset), true));
    let (second_owner, second, second_entry) =
        install_owner(id, state_data(Some(second_offset), true));
    let _registered = Registered([id]);
    first.write().unwrap().bind_object_ref(&first_owner);
    second.write().unwrap().bind_object_ref(&second_owner);
    for entry in [&first_entry, &second_entry] {
        entry.with_module(|module| {
            let model = (module as &mut dyn std::any::Any)
                .downcast_mut::<W3DModelDraw>()
                .unwrap();
            model.cur_state = Some(ActiveModelState::Condition(0));
        });
    }
    // The registry selects the second instance. Each callback must query its
    // own installed entry through the already borrowed Drawable instead.
    for (drawable, entry, expected) in [
        (&first, &first_entry, first_offset),
        (&second, &second_entry, second_offset),
        (&first, &first_entry, first_offset),
    ] {
        crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
        let actual = entry.with_module(|module| {
            let model = (module as &mut dyn std::any::Any)
                .downcast_mut::<W3DModelDraw>()
                .unwrap();
            *model.attach_offset_cache.lock().unwrap()
        });
        assert_eq!(actual, Some(expected));
    }
}
