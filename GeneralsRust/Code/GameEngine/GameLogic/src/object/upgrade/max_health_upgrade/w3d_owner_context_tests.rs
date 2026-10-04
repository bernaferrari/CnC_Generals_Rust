//! hq-5pbog: real CommonINI -> ModuleFactory -> ObjectFactory W3D binding,
//! then actual owner write -> MaxHealthUpgrade -> canonical W3D activation.
use super::*;
use crate::common::{Color, Matrix3D};
use crate::object::draw::w3d_model_draw::{
    W3DModelDraw, register_pristine_bone_lookup_hook, register_sub_object_name_hook,
};
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::Mutex;

// C++ DrawableStatus.h / W3DModelDraw.cpp:2978; exercise the real status bits.
const NO_STATE_PARTICLES: u32 = 0x00000008;

struct AssetHooks;
impl Drop for AssetHooks {
    fn drop(&mut self) {
        register_pristine_bone_lookup_hook(None);
        register_sub_object_name_hook(None);
    }
}

fn authored_model_admitted(name: &str) -> (Admission, Arc<RwLock<Object>>, Arc<ModuleEntry>) {
    assert!(ensure_thing_factory_exists());
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER));
    });
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    // Real authored catalog, before Object parse/creation; no loaded flag.
    assert_eq!(
        crate::object::armor::load_armor_templates_from_str(
            "Armor W3DOwnerContextArmor\nArmor = DEFAULT 100%\nEnd\n",
            None,
        )
        .unwrap(),
        1
    );
    let authored = format!(
        "Object {name}\nKindOf = INERT\n\
ArmorSet\nConditions = NONE\nArmor = W3DOwnerContextArmor\nDamageFX = None\nEnd\n\
Body = ActiveBody ActualBody\nMaxHealth = 100\nInitialHealth = 25\nEnd\n\
Behavior = MaxHealthUpgrade ActualUpgrade\nTriggeredBy = {TRIGGER}\nAddMaxHealth = 50\nChangeType = FULLY_HEAL\nEnd\n\
Draw = W3DModelDraw ActualDraw\nExtraPublicBone = OwnerContextBone\nReceivesDynamicLights = No\n\
DefaultConditionState\nModel = W3DOwnerContextPristine\nEnd\n\
ConditionState = DAMAGED\nModel = W3DOwnerContextDamaged\nEnd\n\
ConditionState = REALLYDAMAGED\nModel = W3DOwnerContextReallyDamaged\nEnd\n\
End\nEnd\n"
    );
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&authored),
        1
    );
    {
        let template = get_thing_factory()
            .unwrap()
            .as_ref()
            .unwrap()
            .find_template(name, false)
            .unwrap();
        let mut entries = template.get_draw_module_info().iter();
        let entry = entries.next().expect("authored Draw descriptor");
        assert!(entries.next().is_none());
        let data = entry
            .data
            .as_ref()
            .as_any()
            .downcast_ref::<crate::object::draw::w3d_model_draw::W3DModelDrawModuleData>()
            .expect("CommonINI must call the actual registered W3D data proc");
        assert_eq!(data.default_state, 0);
        assert_eq!(
            data.condition_states
                .iter()
                .map(|state| state.model_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "w3downercontextpristine",
                "w3downercontextdamaged",
                "w3downercontextreallydamaged"
            ],
        );
        assert_eq!(
            data.condition_states[0].conditions_yes,
            vec![ModelConditionFlags::empty()]
        );
        assert_eq!(
            data.condition_states[1].conditions_yes,
            vec![ModelConditionFlags::DAMAGED]
        );
        assert_eq!(
            data.condition_states[2].conditions_yes,
            vec![ModelConditionFlags::REALLYDAMAGED]
        );
        assert_eq!(
            data.extra_public_bones
                .iter()
                .map(|bone| bone.as_str())
                .collect::<Vec<_>>(),
            vec!["OwnerContextBone"]
        );
        assert!(!data.receives_dynamic_lights);
    }
    let mut admission = Admission(ObjectFactory::new());
    let id = admission
        .0
        .create_object(name, Coord3D::default(), None, ObjectCreationFlags::NO_AI)
        .unwrap();
    let owner = admission
        .0
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(Arc::ptr_eq(
        &owner,
        &OBJECT_REGISTRY.get_object(id).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &owner,
        &TheGameLogic::find_object_by_id(id).unwrap()
    ));
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("MaxHealthUpgrade")
        .unwrap();
    (admission, owner, entry)
}

fn actual_model(drawable: &Arc<RwLock<Drawable>>) -> crate::object::drawable::DrawableModuleHandle {
    let modules = drawable
        .read()
        .unwrap()
        .modules_with_interface(ModuleInterfaceType::DRAW);
    assert_eq!(modules.len(), 1);
    let handle = modules.into_iter().next().unwrap();
    assert_eq!(handle.name().as_str(), "W3DModelDraw");
    assert_eq!(handle.tag().as_str(), "ActualDraw");
    assert!(
        handle
            .with_module_downcast::<W3DModelDraw, _, _>(|_| ())
            .is_some()
    );
    handle
}

fn packed_indicator(color: Color) -> i32 {
    let packed = ((color.r as i32) << 16) | ((color.g as i32) << 8) | color.b as i32;
    if packed == 0 {
        0
    } else {
        packed | 0xFF000000u32 as i32
    }
}

#[test]
fn authored_object_factory_binds_actual_w3d_without_relocking_owner_or_drawable() {
    if !child(concat!(
        module_path!(),
        "::authored_object_factory_binds_actual_w3d_without_relocking_owner_or_drawable"
    )) {
        return;
    }
    let _rules = Rules::install();
    let (_admission, owner, _) = authored_model_admitted("W3DActualFactoryBinding");
    let (id, drawable, indicator, body) = {
        let owner = owner.read().unwrap();
        let night = crate::helpers::TheGlobalData::get()
            .map(|data| data.get_time_of_day() == crate::common::audio::TimeOfDay::Night)
            .unwrap_or(false);
        (
            owner.get_id(),
            owner.get_drawable().unwrap(),
            if night {
                owner.get_night_indicator_color()
            } else {
                owner.get_indicator_color()
            },
            owner.get_body_module().unwrap(),
        )
    };
    {
        let draw = drawable.read().unwrap();
        assert_eq!(draw.get_object_id(), id);
        assert!(Arc::ptr_eq(&draw.get_object().unwrap(), &owner));
    }
    actual_model(&drawable)
        .with_module_downcast::<W3DModelDraw, _, _>(|draw| {
            assert_eq!(draw.owner_id(), Some(id));
            assert!(draw.has_render_model());
            assert_eq!(draw.owner_context_probe().0, "w3downercontextpristine");
            assert_eq!(draw.hex_color(), packed_indicator(indicator));
        })
        .unwrap();
    assert_eq!(body.lock().unwrap().get_health(), 25.0);
    assert_eq!(
        body.lock().unwrap().get_damage_state(),
        BodyDamageType::Damaged
    );
}

#[test]
fn actual_w3d_max_health_reaction_borrows_exact_facts_and_releases_drawable_before_callback() {
    if !child(concat!(
        module_path!(),
        "::actual_w3d_max_health_reaction_borrows_exact_facts_and_releases_drawable_before_callback"
    )) {
        return;
    }
    let _rules = Rules::install();
    let _hooks = AssetHooks;
    let scales = Arc::new(Mutex::new(Vec::<(String, f32)>::new()));
    let observed_scales = scales.clone();
    register_pristine_bone_lookup_hook(Some(Arc::new(move |model, scale, _, bone| {
        if bone == "ownercontextbone" {
            observed_scales
                .lock()
                .unwrap()
                .push((model.to_owned(), scale));
            Some((1, Matrix3D::IDENTITY))
        } else {
            None
        }
    })));
    let (_admission, owner, upgrade) = authored_model_admitted("W3DActualMaxHealthReaction");
    let (drawable, body) = {
        let owner = owner.read().unwrap();
        (
            owner.get_drawable().unwrap(),
            owner.get_body_module().unwrap(),
        )
    };
    let model = actual_model(&drawable);
    // Consume the actual pending recalc using its production routine with
    // Object/Drawable free. This is not an injected field or duplicate runtime.
    model
        .with_module_downcast::<W3DModelDraw, _, _>(|draw| {
            assert!(draw.owner_context_probe().1);
            draw.finish_owner_context_particle_recalc();
            assert!(!draw.owner_context_probe().1);
        })
        .unwrap();
    {
        let mut owner_guard = owner.write().unwrap();
        owner_guard.set_effectively_dead(true);
        {
            let mut draw = drawable.write().unwrap();
            draw.set_instance_scale(2.25);
            draw.set_drawable_status(NO_STATE_PARTICLES);
            draw.set_model_condition_state(ModelConditionFlags::DAMAGED);
        }
        model
            .with_module_downcast::<W3DModelDraw, _, _>(|draw| {
                assert_eq!(draw.owner_context_probe().0, "w3downercontextdamaged");
                assert!(
                    !draw.owner_context_probe().1,
                    "actual NO_STATE_PARTICLES suppresses raising the latch"
                );
            })
            .unwrap();
        {
            let mut draw = drawable.write().unwrap();
            draw.clear_drawable_status(NO_STATE_PARTICLES);
            draw.clear_and_set_model_condition_state(
                ModelConditionFlags::DAMAGED,
                ModelConditionFlags::REALLYDAMAGED,
            );
            draw.set_drawable_status(NO_STATE_PARTICLES);
        }
        assert!(
            model
                .with_module_downcast::<W3DModelDraw, _, _>(|draw| draw.owner_context_probe().1)
                .unwrap()
        );
        let callbacks = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed_callbacks = callbacks.clone();
        let actual_drawable = drawable.clone();
        let actual_body = body.clone();
        let actual_owner = owner.clone();
        register_sub_object_name_hook(Some(Arc::new(move |name| {
            let draw = actual_drawable
                .try_read()
                .expect("W3D activation must run outside the actual Drawable write guard");
            assert!(Arc::ptr_eq(&draw.get_object().unwrap(), &actual_owner));
            assert!(
                actual_owner.try_read().is_err(),
                "the operation still owns the actual Object write borrow"
            );
            assert!(!draw.get_model_conditions().intersects(
                ModelConditionFlags::DAMAGED
                    | ModelConditionFlags::REALLYDAMAGED
                    | ModelConditionFlags::RUBBLE,
            ));
            let body = actual_body
                .try_lock()
                .expect("W3D asset callback must not retain canonical body guard");
            assert_eq!(body.get_health(), 150.0);
            assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
            observed_callbacks.lock().unwrap().push(name.to_owned());
            Vec::new()
        })));
        owner_guard.apply_upgrade_modules(mask());
        assert_completed(&owner_guard, &upgrade);
        assert!(
            callbacks
                .lock()
                .unwrap()
                .iter()
                .any(|name| name == "w3downercontextpristine")
        );
        model
            .with_module_downcast::<W3DModelDraw, _, _>(|draw| {
                assert_eq!(draw.owner_context_probe().0, "w3downercontextpristine");
                assert!(!draw.last_model_conditions().intersects(
                    ModelConditionFlags::DAMAGED
                        | ModelConditionFlags::REALLYDAMAGED
                        | ModelConditionFlags::RUBBLE,
                ));
                assert!(
                    draw.owner_context_probe().1,
                    "CPP disabling state particles does not clear a previously pending latch"
                );
            })
            .unwrap();
        register_sub_object_name_hook(None);
    }
    let scales = scales.lock().unwrap();
    assert!(
        scales
            .iter()
            .any(|(model, scale)| model == "w3downercontextpristine" && *scale == 1.0)
    );
    assert!(
        scales
            .iter()
            .any(|(model, scale)| model == "w3downercontextdamaged" && *scale == 2.25)
    );
    assert!(
        scales
            .iter()
            .any(|(model, scale)| model == "w3downercontextreallydamaged" && *scale == 2.25)
    );
}

// Actual production W3D data proc must work before authored Draw descriptors
// exist; registration order stays unchanged, without a default data shell.
#[test]
fn owned_w3d_data_proc_parses_authored_states_before_draw_descriptor_registration() {
    if !child(concat!(
        module_path!(),
        "::owned_w3d_data_proc_parses_authored_states_before_draw_descriptor_registration"
    )) {
        return;
    }
    use crate::object::draw::w3d_model_draw::W3DModelDrawModuleData;
    use game_engine::common::ini::{INI, INIError};
    use game_engine::common::thing::module::ModuleType;
    use game_engine::common::thing::module_factory::ModuleFactory;
    let mut factory = ModuleFactory::new();
    crate::contain_module_overrides::register_module_overrides(&mut factory).unwrap();
    assert!(factory.descriptors_for_type(ModuleType::Draw).is_empty());
    assert_eq!(
        factory.find_module_interface_mask("W3DModelDraw", ModuleType::Draw),
        ModuleInterfaceType::NONE
    );
    assert!(
        factory.has_module_data_proc("W3DModelDraw", ModuleType::Draw),
        "registered actual data proc precedes descriptor metadata"
    );
    let mut ini = INI::new();
    let data = ini
        .with_inline_source(
            "ExtraPublicBone = OwnerContextBone\nReceivesDynamicLights = No\n\
DefaultConditionState\nModel = W3DOwnerContextPristine\nEnd\n\
ConditionState = DAMAGED\nModel = W3DOwnerContextDamaged\nEnd\n\
ConditionState = REALLYDAMAGED\nModel = W3DOwnerContextReallyDamaged\nEnd\nEnd\n",
            |ini| {
                factory
                    .try_new_module_data_from_ini(
                        Some(ini),
                        "W3DModelDraw",
                        ModuleType::Draw,
                        "AuthoredOwnedDrawData",
                    )
                    .ok_or(INIError::InvalidData)
            },
        )
        .unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .expect("actual registered W3D definition");
    assert_eq!(data.default_state, 0);
    assert_eq!(
        data.condition_states
            .iter()
            .map(|state| state.model_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "w3downercontextpristine",
            "w3downercontextdamaged",
            "w3downercontextreallydamaged"
        ]
    );
    assert_eq!(
        data.condition_states[0].conditions_yes,
        vec![ModelConditionFlags::empty()]
    );
    assert_eq!(
        data.condition_states[1].conditions_yes,
        vec![ModelConditionFlags::DAMAGED]
    );
    assert_eq!(
        data.condition_states[2].conditions_yes,
        vec![ModelConditionFlags::REALLYDAMAGED]
    );
    assert_eq!(
        data.extra_public_bones
            .iter()
            .map(|bone| bone.as_str())
            .collect::<Vec<_>>(),
        vec!["OwnerContextBone"]
    );
    assert!(!data.receives_dynamic_lights);
    assert_eq!(
        data.module_tag_name_key,
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            "AuthoredOwnedDrawData"
        )
    );
    assert!(
        factory.descriptors_for_type(ModuleType::Draw).is_empty(),
        "creating real data does not reorder/add authored descriptors"
    );
    assert_eq!(
        factory.find_module_interface_mask("W3DModelDraw", ModuleType::Draw),
        ModuleInterfaceType::NONE
    );
}
