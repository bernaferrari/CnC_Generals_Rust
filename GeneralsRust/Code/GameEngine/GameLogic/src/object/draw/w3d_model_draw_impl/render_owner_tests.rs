use super::*;

#[test]
fn installed_draw_with_owner_write_guard_does_not_reenter_drawable() {
    const CHILD: &str = "GENERALS_W3D_DRAW_OWNER_REENTRY_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!()
        .strip_prefix(prefix)
        .expect("test module path includes crate name");
    let test_name =
        format!("{module}::installed_draw_with_owner_write_guard_does_not_reenter_drawable");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&test_name, CHILD)
    {
        return;
    }

    let _serial = crate::test_sync::lock();
    const OWNER: ObjectID = 0xA77A_10F0;
    let (_owner, drawable, entry) = install_owner(OWNER, model_data(true));
    let _registered = Registered([OWNER]);

    // The production GameClient driver has already taken Drawable::write before
    // invoking Draw::draw. This marker proves the child reaches the real installed
    // callback immediately before its first W3DModelDraw owner query:
    // owner_should_animate -> with_owner_drawable -> drawable.read.
    eprintln!(
        "render child: before actual Drawable::draw; first base owner lookup is owner_should_animate"
    );
    let mut drawable_guard = drawable.write().expect("fixture Drawable write guard");
    crate::drawable::Drawable::draw(&mut *drawable_guard, None);
    drop(drawable_guard);

    // This inert installed module has no active model/render object. CPP
    // W3DModelDraw.cpp:2072-2078 guards adjustTransformMtx with m_renderObject;
    // this control must return without querying or priming attachment state.
    // Valid-model admission and its cache query are separate controls.
    let cached = entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .expect("installed W3DModelDraw");
        *model.attach_offset_cache.lock().expect("cache")
    });
    assert_eq!(
        cached, None,
        "no active render object must not prime the cache"
    );
}

#[test]
fn animation_gate_uses_bound_object_with_reused_id_and_expired_owner() {
    let _serial = crate::test_sync::lock();
    const ID: ObjectID = 0xA77A_10F1;
    let (first, first_drawable, _) = install_owner(ID, model_data(false));
    first_drawable.write().unwrap().bind_object_ref(&first);
    first.write().unwrap().set_script_status(
        crate::object::ObjectScriptStatusBit::ScriptUnderpowered,
        true,
    );
    // The registry now contains the second object with exactly the same ID.
    let (second, second_drawable, _) = install_owner(ID, model_data(false));
    let _registered = Registered([ID]);
    second_drawable.write().unwrap().bind_object_ref(&second);
    assert!(!first_drawable.read().unwrap().get_should_animate(true));
    assert!(first_drawable.read().unwrap().get_should_animate(false));
    assert!(second_drawable.read().unwrap().get_should_animate(true));
    first
        .write()
        .unwrap()
        .clear_script_status(crate::object::ObjectScriptStatusBit::ScriptUnderpowered);
    second.write().unwrap().set_script_status(
        crate::object::ObjectScriptStatusBit::ScriptUnderpowered,
        true,
    );
    assert!(first_drawable.read().unwrap().get_should_animate(true));
    assert!(!second_drawable.read().unwrap().get_should_animate(true));
    drop(first);
    // CPP Drawable.cpp:604-642: no bound Object means animate, even if the ID is reused.
    assert!(first_drawable.read().unwrap().get_should_animate(true));
}

#[test]
fn installed_empty_model_transition_preserves_owner_pause_and_pending_state() {
    const CHILD: &str = "GENERALS_W3D_EMPTY_MODEL_TRANSITION_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!().strip_prefix(prefix).unwrap();
    let test_name = format!(
        "{module}::installed_empty_model_transition_preserves_owner_pause_and_pending_state"
    );
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&test_name, CHILD)
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    const ID: ObjectID = 0xA77A_10F2;
    let mut data = model_data(true);
    data.animations_require_power = true;
    let (owner, drawable, entry) = install_owner(ID, data);
    let _registered = Registered([ID]);
    drawable.write().unwrap().bind_object_ref(&owner);
    drawable.write().unwrap().set_instance_scale(2.5);
    owner.write().unwrap().set_script_status(
        crate::object::ObjectScriptStatusBit::ScriptUnderpowered,
        true,
    );
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        model.current_anim_complete = true;
        model.next_state = Some(0);
    });
    eprintln!("render child: before installed empty-model pending transition");
    crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        assert_eq!(model.cur_state, Some(ActiveModelState::Condition(0)));
        assert_eq!(model.next_state, None);
        assert_eq!(model.which_anim_in_cur_state, -1);
        // CPP render replacement resets the pause latch even when the incoming pause decision is true.
        assert!(!model.pause_animation);
        assert_eq!(*model.attach_offset_cache.lock().unwrap(), None);
    });
    // The following draw samples the current Object again rather than retaining a decision.
    crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        assert!(model.pause_animation);
        assert_eq!(*model.attach_offset_cache.lock().unwrap(), None);
    });
}

#[test]
fn installed_model_hiding_is_independent_from_drawable_visibility() {
    let _serial = crate::test_sync::lock();
    let mut drawable = Drawable::new(
        0xD30_2,
        INVALID_ID,
        "HidePropagation".into(),
        DrawableType::Static,
    );
    let data = W3DModelDrawModuleData::new();
    let entry = drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        "HidePropagation".into(),
        Arc::new(data.clone()),
        Box::new(W3DModelDraw::new(data)),
    );
    for (hidden, visible) in [(false, false), (true, false), (true, true), (false, true)] {
        drawable.set_drawable_hidden(hidden).unwrap();
        drawable.set_visible(visible);
        let model_hidden = entry.with_module(|module| {
            (module as &mut dyn std::any::Any)
                .downcast_mut::<W3DModelDraw>()
                .unwrap()
                .hidden
        });
        assert_eq!(model_hidden, hidden);
        assert_eq!(drawable.is_drawable_effectively_hidden(), hidden);
        assert_eq!(drawable.is_currently_visible(), visible && !hidden);
    }
}
