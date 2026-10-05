//! Actual map-property mutation under the admitted driving Object write guard.
//! CPP Object.cpp:3440-3451; BodyModule.h:152 (SAME_CURRENTHEALTH default);
//! ActiveBody.cpp:855-922,1188-1227; ImmortalBody.cpp:31-37.
use super::*;
use crate::common::{BodyDamageType, ModelConditionFlags, ObjectStatusMaskType};
use crate::helpers::TheGameLogic;
use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::global_data;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::sync::{Arc, RwLock};

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_MAP_BODY_OWNER_CHILD"
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}
struct Rules {
    small: String,
    count: i32,
    damaged: f32,
    really: f32,
}
impl Rules {
    fn install() -> Self {
        let mut data = global_data::write();
        let old = Self {
            small: data.auto_fire_particle_small_system.clone(),
            count: data.auto_fire_particle_small_max,
            damaged: data.unit_damaged_thresh,
            really: data.unit_really_damaged_thresh,
        };
        // Installed AFTER creation to isolate the map callback. This nonempty
        // grouping reaches OLD owner.read before template/bone resolution.
        // Successful asset-backed particle creation is covered separately.
        data.auto_fire_particle_small_system = "MapOwnerFireWitness".to_owned();
        data.auto_fire_particle_small_max = 1;
        data.unit_damaged_thresh = 0.5;
        data.unit_really_damaged_thresh = 0.1;
        old
    }
}
impl Drop for Rules {
    fn drop(&mut self) {
        let mut data = global_data::write();
        data.auto_fire_particle_small_system = self.small.clone();
        data.auto_fire_particle_small_max = self.count;
        data.unit_damaged_thresh = self.damaged;
        data.unit_really_damaged_thresh = self.really;
    }
}
struct Admission(ObjectFactory);
impl Drop for Admission {
    fn drop(&mut self) {
        let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
        self.0.clear_all_objects(&mut logic).unwrap();
    }
}
fn admitted(
    name: &str,
    body_class: &str,
) -> (Admission, Arc<RwLock<Object>>, Arc<RwLock<Drawable>>) {
    assert!(ensure_thing_factory_exists());
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let authored = format!(
        "Object {name}\nKindOf = INERT\nBody = {body_class} ActualBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
    );
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&authored),
        1
    );
    let mut admission = Admission(ObjectFactory::new());
    let id = admission
        .0
        .create_object(
            name,
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_AI | ObjectCreationFlags::NO_DRAWABLE,
        )
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
    assert!(owner.read().unwrap().get_body_module().is_some());
    // A real Drawable witnesses the damage-state callback; no render asset or
    // fake body state is installed. The body itself is the authored factory one.
    let drawable = Arc::new(RwLock::new(Drawable::new(
        0x7A3D_1001,
        id,
        String::new(),
        DrawableType::Static,
    )));
    owner
        .write()
        .unwrap()
        .set_drawable(Some(Arc::clone(&drawable)));
    (admission, owner, drawable)
}
fn properties(maximum: Option<i32>, percent: Option<i32>) -> Dict {
    let mut props = Dict::new();
    if let Some(value) = maximum {
        props.set_int(crate::common::well_known_keys::key_object_max_hps(), value);
    }
    if let Some(value) = percent {
        props.set_int(
            crate::common::well_known_keys::key_object_initial_health(),
            value,
        );
    }
    props
}
fn health(owner: &Object) -> (f32, f32, f32, f32, BodyDamageType) {
    let body = owner.get_body_module().unwrap();
    let body = body.lock().unwrap();
    (
        body.get_health(),
        body.get_max_health(),
        body.get_initial_health(),
        body.get_previous_health(),
        body.get_damage_state(),
    )
}
#[test]
fn map_maximum_uses_cpp_same_current_health_default() {
    if !child(concat!(
        module_path!(),
        "::map_maximum_uses_cpp_same_current_health_default"
    )) {
        return;
    }
    let (_admission, owner, _) = admitted("MapMaximumOwner", "ActiveBody");
    let mut owner = owner.write().unwrap();
    owner.update_obj_values_from_map_properties(&properties(Some(200), None));
    // C++ SAME_CURRENTHEALTH performs no internalChangeHealth unless capped.
    assert_eq!(health(&owner).0, 100.0);
    assert_eq!(health(&owner).1, 200.0);
    assert_eq!(health(&owner).2, 200.0);
    assert!(!owner.is_effectively_dead());
}
#[test]
fn map_maximum_then_initial_percent_reacts_on_exact_admitted_owner() {
    if !child(concat!(
        module_path!(),
        "::map_maximum_then_initial_percent_reacts_on_exact_admitted_owner"
    )) {
        return;
    }
    let (_admission, owner, drawable) = admitted("MapCombinedOwner", "ActiveBody");
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.set_effectively_dead(true);
    owner.update_obj_values_from_map_properties(&properties(Some(200), Some(25)));
    assert_eq!(
        health(&owner),
        (50.0, 200.0, 200.0, 100.0, BodyDamageType::Damaged)
    );
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::DAMAGED)
    );
    assert!(!owner.is_effectively_dead());
    #[cfg(any(debug_assertions, feature = "internal"))]
    assert!(!owner.has_died_already);
    assert!(!owner.is_destroyed());
}
#[test]
fn map_initial_percent_preserves_construction_visual_gate_and_updates_dead_bit() {
    if !child(concat!(
        module_path!(),
        "::map_initial_percent_preserves_construction_visual_gate_and_updates_dead_bit"
    )) {
        return;
    }
    let (_admission, owner, drawable) = admitted("MapConstructionOwner", "ActiveBody");
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.set_status(ObjectStatusMaskType::UNDER_CONSTRUCTION, true);
    owner.set_effectively_dead(true);
    owner.update_obj_values_from_map_properties(&properties(None, Some(25)));
    assert_eq!(
        health(&owner),
        (25.0, 100.0, 100.0, 100.0, BodyDamageType::Damaged)
    );
    assert!(!drawable.read().unwrap().get_model_conditions().intersects(
        ModelConditionFlags::DAMAGED
            | ModelConditionFlags::REALLYDAMAGED
            | ModelConditionFlags::RUBBLE
    ));
    assert!(!owner.is_effectively_dead());
    #[cfg(any(debug_assertions, feature = "internal"))]
    assert!(!owner.has_died_already);
}
#[test]
fn map_zero_maximum_caps_and_updates_rubble_without_on_die() {
    if !child(concat!(
        module_path!(),
        "::map_zero_maximum_caps_and_updates_rubble_without_on_die"
    )) {
        return;
    }
    let (_admission, owner, drawable) = admitted("MapZeroMaximumOwner", "ActiveBody");
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.update_obj_values_from_map_properties(&properties(Some(0), None));
    assert_eq!(
        health(&owner),
        (0.0, 0.0, 0.0, 100.0, BodyDamageType::Rubble)
    );
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RUBBLE)
    );
    assert!(owner.is_effectively_dead());
    #[cfg(any(debug_assertions, feature = "internal"))]
    assert!(!owner.has_died_already);
    assert!(!owner.is_destroyed());
}
#[test]
fn map_initial_percent_forwards_immortal_floor_to_same_body() {
    if !child(concat!(
        module_path!(),
        "::map_initial_percent_forwards_immortal_floor_to_same_body"
    )) {
        return;
    }
    let (_admission, owner, drawable) = admitted("MapImmortalOwner", "ImmortalBody");
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    let identity = owner.get_body_module().unwrap();
    owner.update_obj_values_from_map_properties(&properties(None, Some(0)));
    assert!(Arc::ptr_eq(&identity, &owner.get_body_module().unwrap()));
    assert_eq!(
        health(&owner),
        (1.0, 100.0, 100.0, 100.0, BodyDamageType::ReallyDamaged)
    );
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::REALLYDAMAGED)
    );
    assert!(!owner.is_effectively_dead());
    #[cfg(any(debug_assertions, feature = "internal"))]
    assert!(!owner.has_died_already);
}
#[test]
fn map_inactive_body_remains_inert() {
    if !child(concat!(module_path!(), "::map_inactive_body_remains_inert")) {
        return;
    }
    let (_admission, owner, drawable) = admitted("MapInactiveOwner", "InactiveBody");
    let mut owner = owner.write().unwrap();
    let before = health(&owner);
    let conditions = drawable.read().unwrap().get_model_conditions();
    let dead = owner.is_effectively_dead();
    owner.update_obj_values_from_map_properties(&properties(Some(200), Some(25)));
    assert_eq!(health(&owner), before);
    assert_eq!(drawable.read().unwrap().get_model_conditions(), conditions);
    assert_eq!(owner.is_effectively_dead(), dead);
    #[cfg(any(debug_assertions, feature = "internal"))]
    assert!(!owner.has_died_already);
}
