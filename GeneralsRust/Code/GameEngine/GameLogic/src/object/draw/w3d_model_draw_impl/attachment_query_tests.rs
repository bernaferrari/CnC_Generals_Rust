use crate::object::drawable::{Drawable, DrawableArcExt, DrawableExt, DrawableType};
use crate::object::registry::OBJECT_REGISTRY;
use std::sync::{Arc, RwLock};

struct Registered<const N: usize>([ObjectID; N]);
impl<const N: usize> Drop for Registered<N> {
    fn drop(&mut self) {
        for id in self.0 {
            OBJECT_REGISTRY.unregister_object(id);
        }
    }
}

fn model_data(with_state: bool) -> W3DModelDrawModuleData {
    let mut data = W3DModelDrawModuleData::new();
    data.attach_to_drawable_bone = AsciiString::from("attach");
    data.attach_to_drawable_bone_offset = Coord3D::new(90.0, 91.0, 92.0);
    if with_state {
        data.default_state = 0;
        let attach_key = NameKeyGenerator::name_to_key("attach");
        let turret_key = NameKeyGenerator::name_to_key("turret");
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.pristine_bones.insert(
            attach_key,
            PristineBoneInfo {
                transform: Matrix3D::from_translation(Coord3D::new(4.0, 5.0, 6.0)),
                bone_index: 7,
            },
        );
        state.pristine_bones.insert(
            turret_key,
            PristineBoneInfo {
                transform: Matrix3D::from_translation(Coord3D::new(1.0, 2.0, 3.0)),
                bone_index: 8,
            },
        );
        state.turrets.push(TurretInfo {
            turret_angle_name_key: turret_key,
            ..TurretInfo::new()
        });
        state.weapon_barrels[0].push(WeaponBarrelInfo {
            projectile_offset_mtx: Matrix3D::from_translation(Coord3D::new(10.0, 0.0, 0.0)),
            ..WeaponBarrelInfo::new()
        });
        data.condition_states.push(state);
    }
    data
}

fn install_owner(
    id: ObjectID,
    data: W3DModelDrawModuleData,
) -> (
    Arc<RwLock<crate::object::Object>>,
    Arc<RwLock<Drawable>>,
    crate::object::drawable::DrawableModuleHandle,
) {
    let mut model = W3DModelDraw::new(data.clone());
    model.owner_id = Some(id);
    let mut drawable = Drawable::new(id, id, format!("AttachCache{id}"), DrawableType::Static);
    let entry = drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        format!("AttachCache{id}").into(),
        Arc::new(data),
        Box::new(model),
    );
    let drawable = Arc::new(RwLock::new(drawable));
    let mut owner = crate::object::Object::new_test(id, 100.0);
    owner.set_drawable(Some(Arc::clone(&drawable)));
    let owner = Arc::new(RwLock::new(owner));
    OBJECT_REGISTRY.register_object(id, &owner);
    (owner, drawable, entry)
}

use super::*;
use crate::common::WeaponSlotType;

#[test]
fn installed_projectile_attach_cache_miss_does_not_reenter_draw_entry() {
    const CHILD: &str = "GENERALS_W3D_ATTACH_CACHE_REENTRY_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!()
        .strip_prefix(prefix)
        .expect("test module path includes crate name");
    let test_name =
        format!("{module}::installed_projectile_attach_cache_miss_does_not_reenter_draw_entry");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&test_name, CHILD)
    {
        return;
    }

    let _serial = crate::test_sync::lock();
    const OWNER_WITH_STATE: ObjectID = 0xA77A_1001;
    const OWNER_WITHOUT_STATE: ObjectID = 0xA77A_1002;
    eprintln!("attach-cache child: creating no-state early-return control");
    let (_owner_without_state, drawable_without_state, _entry_without_state) =
        install_owner(OWNER_WITHOUT_STATE, model_data(false));
    assert!(
        drawable_without_state
            .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary,)
            .is_none()
    );

    eprintln!("attach-cache child: creating installed cold-cache owner");
    let (_owner, drawable, entry) = install_owner(OWNER_WITH_STATE, model_data(true));
    let _registered = Registered([OWNER_WITH_STATE, OWNER_WITHOUT_STATE]);

    // Invalid slot must return before querying/priming the attach cache. Change
    // the bone afterward; the valid query must use this changed pristine value.
    let invalid_slot_result = entry
        .with_object_draw_interface(|draw| {
            let mut launch = Matrix3D::IDENTITY;
            let mut turret = Coord3D::origin();
            let mut pitch = Coord3D::origin();
            draw.get_projectile_launch_offset(
                &ModelConditionFlags::empty(),
                WEAPONSLOT_COUNT,
                0,
                &mut launch,
                TurretType::Primary,
                &mut turret,
                &mut pitch,
            )
        })
        .expect("DRAW entry implements ObjectDrawInterface");
    assert!(!invalid_slot_result);
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .expect("installed W3DModelDraw");
        model.data.condition_states[0]
            .pristine_bones
            .get_mut(&NameKeyGenerator::name_to_key("attach"))
            .expect("attach bone fixture")
            .transform = Matrix3D::from_translation(Coord3D::new(7.0, 8.0, 9.0));
    });

    eprintln!("attach-cache child: entering actual DrawableArcExt cold-cache query");
    // OLD hangs here: this draw entry is already locked; a cache miss calls
    // Drawable::get_pristine_bone_positions, which locks this same entry again.
    let first = drawable
        .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
        .expect("valid state and barrel provide a projectile offset");
    assert_eq!(first.transform.w_axis.x, 10.0);
    assert_eq!(first.transform.w_axis.y, 0.0);
    assert_eq!(first.transform.w_axis.z, 0.0);
    assert_eq!(first.turret_rot_pos, Coord3D::new(8.0, 10.0, 12.0));
    assert_eq!(first.turret_pitch_pos, Coord3D::new(7.0, 8.0, 9.0));

    // Preserve the existing runtime cache on repeated queries. C++ stores its
    // cache on shared module data; definition sharing is a separate migration.
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .expect("installed W3DModelDraw");
        model.data.condition_states[0]
            .pristine_bones
            .get_mut(&NameKeyGenerator::name_to_key("attach"))
            .expect("attach bone fixture")
            .transform = Matrix3D::from_translation(Coord3D::new(11.0, 12.0, 13.0));
    });
    let second = drawable
        .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
        .expect("cached module still supplies its projectile offset");
    assert_eq!(second.turret_rot_pos, first.turret_rot_pos);
    assert_eq!(second.turret_pitch_pos, first.turret_pitch_pos);
}

fn assert_cold(draw: &crate::object::drawable::DrawableModuleHandle) {
    draw.with_module(|module| {
        let draw = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        assert!(draw.attach_offset_cache.lock().unwrap().is_none());
    });
}

#[test]
fn invalid_turret_skips_attachment_and_missing_bone_caches_zero() {
    const CHILD: &str = "GENERALS_W3D_INVALID_TURRET_CHILD";
    let test = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        &format!("{test}::invalid_turret_skips_attachment_and_missing_bone_caches_zero"),
        CHILD,
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0xA77A_1003;
    let (_owner, drawable, entry) = install_owner(id, model_data(true));
    let _registered = Registered([id]);
    eprintln!("invalid-turret child: entering cold invalid-turret query");
    let launch = drawable
        .get_projectile_launch_offset(WeaponSlotType::Primary, -1, TurretType::Invalid)
        .unwrap();
    assert_eq!(
        launch.transform.w_axis.truncate(),
        Coord3D::new(10.0, 0.0, 0.0)
    );
    assert_eq!(launch.turret_rot_pos, Coord3D::origin());
    assert_cold(&entry);
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        model.data.condition_states[0]
            .pristine_bones
            .remove(&NameKeyGenerator::name_to_key("attach"));
    });
    let launch = drawable
        .get_projectile_launch_offset(WeaponSlotType::Primary, 99, TurretType::Primary)
        .unwrap();
    assert_eq!(launch.turret_rot_pos, Coord3D::new(1.0, 2.0, 3.0));
    assert_eq!(launch.turret_pitch_pos, Coord3D::origin());
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        assert_eq!(
            *model.attach_offset_cache.lock().unwrap(),
            Some(Coord3D::origin())
        );
        model.data.condition_states[0].pristine_bones.insert(
            NameKeyGenerator::name_to_key("attach"),
            PristineBoneInfo {
                transform: Matrix3D::from_translation(Coord3D::new(90.0, 90.0, 90.0)),
                bone_index: 7,
            },
        );
    });
    assert_eq!(
        drawable
            .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
            .unwrap()
            .turret_rot_pos,
        launch.turret_rot_pos
    );
}

#[test]
fn empty_barrels_still_prime_attachment_before_returning_no_launch() {
    const CHILD: &str = "GENERALS_W3D_EMPTY_BARRELS_CHILD";
    let test = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        &format!("{test}::empty_barrels_still_prime_attachment_before_returning_no_launch"),
        CHILD,
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0xA77A_1004;
    let mut data = model_data(true);
    data.condition_states[0].weapon_barrels[0].clear();
    let (_owner, drawable, entry) = install_owner(id, data);
    let _registered = Registered([id]);
    assert!(
        drawable
            .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
            .is_none()
    );
    entry.with_module(|module| {
        let model = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        assert_eq!(
            *model.attach_offset_cache.lock().unwrap(),
            Some(Coord3D::new(4.0, 5.0, 6.0))
        );
        model.data.condition_states[0].weapon_barrels[0].push(WeaponBarrelInfo::new());
        model.data.condition_states[0]
            .pristine_bones
            .get_mut(&NameKeyGenerator::name_to_key("attach"))
            .unwrap()
            .transform = Matrix3D::from_translation(Coord3D::new(100.0, 100.0, 100.0));
    });
    assert_eq!(
        drawable
            .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
            .unwrap()
            .turret_rot_pos,
        Coord3D::new(5.0, 7.0, 9.0)
    );
}

#[test]
fn concurrent_same_id_drawables_resolve_their_own_attachment() {
    const CHILD: &str = "GENERALS_W3D_SIBLING_ATTACH_CHILD";
    let test = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        &format!("{test}::concurrent_same_id_drawables_resolve_their_own_attachment"),
        CHILD,
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0xA77A_1005;
    let (_first_owner, first, _) = install_owner(id, model_data(true));
    let mut second_data = model_data(true);
    second_data.condition_states[0]
        .pristine_bones
        .get_mut(&NameKeyGenerator::name_to_key("attach"))
        .unwrap()
        .transform = Matrix3D::from_translation(Coord3D::new(20.0, 30.0, 40.0));
    let (_second_owner, second, _) = install_owner(id, second_data);
    let _registered = Registered([id]);
    // Ambient registry now selects the second owner. Both query drivers must
    // still select their own installed entries and first pristine result.
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|index| {
            let drawable = Arc::clone(if index % 2 == 0 { &first } else { &second });
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let expected = if index % 2 == 0 {
                    Coord3D::new(5.0, 7.0, 9.0)
                } else {
                    Coord3D::new(21.0, 32.0, 43.0)
                };
                for _ in 0..64 {
                    assert_eq!(
                        drawable
                            .get_projectile_launch_offset(
                                WeaponSlotType::Primary,
                                0,
                                TurretType::Primary
                            )
                            .unwrap()
                            .turret_rot_pos,
                        expected
                    );
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn dependency_and_supply_modules_use_the_driving_drawable_query() {
    const CHILD: &str = "GENERALS_W3D_WRAPPER_ATTACH_CHILD";
    let test = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        &format!("{test}::dependency_and_supply_modules_use_the_driving_drawable_query"),
        CHILD,
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    use crate::object::draw::{
        W3DDependencyModelDraw, W3DDependencyModelDrawModuleData, W3DSupplyDraw,
        W3DSupplyDrawModuleData,
    };
    let ids = [0xA77A_1006, 0xA77A_1007];
    let _registered = Registered(ids);
    for (index, id) in ids.into_iter().enumerate() {
        let (name, data, module): (&str, Arc<dyn ModuleData>, Box<dyn Module>) = if index == 0 {
            let mut data = W3DDependencyModelDrawModuleData::new();
            data.base = model_data(true);
            let mut module = W3DDependencyModelDraw::new(data.clone());
            module.bind_owner_id(id);
            ("W3DDependencyModelDraw", Arc::new(data), Box::new(module))
        } else {
            let mut data = W3DSupplyDrawModuleData::new();
            data.base = model_data(true);
            let mut module = W3DSupplyDraw::new(data.clone());
            module.bind_owner_id(id);
            ("W3DSupplyDraw", Arc::new(data), Box::new(module))
        };
        let mut drawable = Drawable::new(id, id, name.to_owned(), DrawableType::Static);
        drawable.add_module(
            ModuleInterfaceType::DRAW,
            name.into(),
            "AttachWrapper".into(),
            data,
            module,
        );
        let drawable = Arc::new(RwLock::new(drawable));
        let mut owner = crate::object::Object::new_test(id, 100.0);
        owner.set_drawable(Some(Arc::clone(&drawable)));
        let owner = Arc::new(RwLock::new(owner));
        OBJECT_REGISTRY.register_object(id, &owner);
        let launch = drawable
            .get_projectile_launch_offset(WeaponSlotType::Primary, 0, TurretType::Primary)
            .unwrap();
        assert_eq!(launch.turret_rot_pos, Coord3D::new(5.0, 7.0, 9.0));
        assert_eq!(launch.turret_pitch_pos, Coord3D::new(4.0, 5.0, 6.0));
    }
}
