//! C++ ActiveBody.cpp:873-922,1188-1227: actual owner health operation,
//! construction gate, reaction -> particles -> effectivelyDead. These tests
//! exercise the canonical authored body and upgrade, not a test body cache.
use super::*;
use crate::common::ModelConditionFlags;
use crate::common::{BodyDamageType, Coord3D, ObjectStatusMaskType};
use crate::helpers::TheGameLogic;
use crate::helpers::TheThingFactory;
use crate::object::ModuleEntry;
use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::sync::RwLock;
const TRIGGER: &str = "Upgrade_MaxHealthBodyCallback";
fn mask() -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(TRIGGER).to_bits())
}
use game_engine::common::global_data;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_MAX_HEALTH_BODY_CALLBACK_CHILD"
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

// Exact scoped rule restoration; no readiness flag or bulk game reset.
struct Rules {
    small: String,
    count: i32,
    damaged: f32,
    really: f32,
}
impl Rules {
    fn install() -> Self {
        let mut rules = global_data::write();
        let old = Self {
            small: rules.auto_fire_particle_small_system.clone(),
            count: rules.auto_fire_particle_small_max,
            damaged: rules.unit_damaged_thresh,
            really: rules.unit_really_damaged_thresh,
        };
        rules.auto_fire_particle_small_system = "MaxHealthOwnerAutoFire".to_owned();
        rules.auto_fire_particle_small_max = 1;
        rules.unit_damaged_thresh = 0.5;
        rules.unit_really_damaged_thresh = 0.1;
        old
    }
}
impl Drop for Rules {
    fn drop(&mut self) {
        let mut rules = global_data::write();
        rules.auto_fire_particle_small_system = self.small.clone();
        rules.auto_fire_particle_small_max = self.count;
        rules.unit_damaged_thresh = self.damaged;
        rules.unit_really_damaged_thresh = self.really;
    }
}
struct Admission(ObjectFactory);
impl Drop for Admission {
    fn drop(&mut self) {
        let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
        self.0.clear_all_objects(&mut logic).unwrap();
    }
}
fn authored_admitted(name: &str) -> (Admission, Arc<RwLock<Object>>, Arc<ModuleEntry>) {
    let authored = format!(
        "Object {name}\nKindOf = INERT\nBody = ActiveBody ActualBody\nMaxHealth = 100\nInitialHealth = 25\nEnd\nBehavior = MaxHealthUpgrade ActualUpgrade\nTriggeredBy = {TRIGGER}\nAddMaxHealth = 50\nChangeType = FULLY_HEAL\nEnd\nEnd\n"
    );
    let admitted = admitted_from_ini(name, &authored);
    let body = admitted.1.read().unwrap().get_body_module().unwrap();
    assert_eq!(body.lock().unwrap().get_health(), 25.0);
    assert_eq!(
        body.lock().unwrap().get_damage_state(),
        BodyDamageType::Damaged
    );
    admitted
}

fn admitted_from_ini(
    name: &str,
    authored: &str,
) -> (Admission, Arc<RwLock<Object>>, Arc<ModuleEntry>) {
    assert!(ensure_thing_factory_exists());
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER));
    });
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(authored),
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
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("MaxHealthUpgrade")
        .unwrap();
    (admission, owner, entry)
}
fn assert_completed(owner: &Object, entry: &ModuleEntry) {
    let body = owner.get_body_module().unwrap();
    let body = body.lock().unwrap();
    assert_eq!(body.get_health(), 150.0);
    assert_eq!(body.get_max_health(), 150.0);
    assert_eq!(body.get_initial_health(), 150.0);
    assert_eq!(body.get_previous_health(), 25.0);
    assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
    assert!(
        !owner.is_effectively_dead(),
        "CPP updates actual owner after visual callbacks"
    );
    assert!(entry.with_module(|module| {
        module
            .as_any()
            .downcast_ref::<MaxHealthUpgrade>()
            .unwrap()
            .applied
    }));
}
#[test]
fn admitted_max_health_particle_transition_reuses_actual_write_borrowed_owner() {
    if !child(concat!(
        module_path!(),
        "::admitted_max_health_particle_transition_reuses_actual_write_borrowed_owner"
    )) {
        return;
    }
    let (_admission, owner, entry) = authored_admitted("MaxHealthAdmittedCallback");
    // Install after construction so this regression isolates upgrade, not init.
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.set_effectively_dead(true);
    owner.apply_upgrade_modules(mask());
    assert_completed(&owner, &entry);
}
#[test]
fn admitted_under_construction_max_health_skips_visuals_but_updates_owner_dead_bit() {
    if !child(concat!(
        module_path!(),
        "::admitted_under_construction_max_health_skips_visuals_but_updates_owner_dead_bit"
    )) {
        return;
    }
    let (_admission, owner, entry) = authored_admitted("MaxHealthBuildingCallback");
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.set_status(ObjectStatusMaskType::UNDER_CONSTRUCTION, true);
    owner.set_effectively_dead(true);
    owner.apply_upgrade_modules(mask());
    assert_completed(&owner, &entry);
}

#[test]
fn admitted_max_health_reacts_on_actual_drawable_before_clearing_dead_flag() {
    if !child(concat!(
        module_path!(),
        "::admitted_max_health_reacts_on_actual_drawable_before_clearing_dead_flag"
    )) {
        return;
    }
    let (_admission, owner, entry) = authored_admitted("MaxHealthDrawableCallback");
    let id = owner.read().unwrap().get_id();
    let drawable = Arc::new(RwLock::new(Drawable::new(
        0x7A3C_2211,
        id,
        String::new(),
        DrawableType::Static,
    )));
    drawable
        .write()
        .unwrap()
        .set_model_condition_state(ModelConditionFlags::DAMAGED);
    owner.write().unwrap().set_drawable(Some(drawable.clone()));
    let _rules = Rules::install();
    let mut owner = owner.write().unwrap();
    owner.set_effectively_dead(true);
    owner.apply_upgrade_modules(mask());
    assert_completed(&owner, &entry);
    assert!(
        !drawable.read().unwrap().get_model_conditions().intersects(
            ModelConditionFlags::DAMAGED
                | ModelConditionFlags::REALLYDAMAGED
                | ModelConditionFlags::RUBBLE
        ),
        "C++ evaluateVisualCondition must react on the actual supplied owner's Drawable"
    );
}

#[test]
fn admitted_structure_max_health_cap_applies_rubble_pose_before_visuals() {
    if !child(concat!(
        module_path!(),
        "::admitted_structure_max_health_cap_applies_rubble_pose_before_visuals"
    )) {
        return;
    }
    let name = "MaxHealthStructureRubble";
    let authored = format!(
        "Object {name}\nKindOf = STRUCTURE IMMOBILE\nGeometry = SPHERE\nGeometryMajorRadius = 5\nGeometryMinorRadius = 5\nGeometryHeight = 20\nGeometryIsSmall = No\nStructureRubbleHeight = 3\nBody = StructureBody ActualBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = MaxHealthUpgrade ActualUpgrade\nTriggeredBy = {TRIGGER}\nAddMaxHealth = -100\nChangeType = SAME_CURRENTHEALTH\nEnd\nEnd\n"
    );
    let (_admission, owner, _) = admitted_from_ini(name, &authored);
    let pos = Coord3D::new(35.0, 35.0, 0.0);
    owner.write().unwrap().set_position(&pos).unwrap();
    let mut terrain = crate::terrain::TerrainLogic::new();
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 16;
    map.height = 16;
    map.heightmap = vec![0; 256];
    map.boundaries = vec![crate::common::ICoord2D::new(16, 16)];
    terrain.load_map_data(map);
    let pf = crate::ai::the_ai().read().unwrap().pathfinder().unwrap();
    pf.write().unwrap().rebuild_from_terrain(&terrain);
    assert!(pf.read().unwrap().is_map_ready());
    assert_eq!(
        owner
            .read()
            .unwrap()
            .get_geometry_info()
            .get_max_height_above_position(),
        20.0
    );
    {
        let mut owner = owner.write().unwrap();
        owner.apply_upgrade_modules(mask());
        assert!(
            owner
                .get_status_bits()
                .contains(ObjectStatusMaskType::NO_COLLISIONS),
            "C++ setCorrectDamageState applies rubble during the health transition"
        );
        assert_eq!(
            owner.get_geometry_info().get_max_height_above_position(),
            3.0
        );
        assert!(owner.is_effectively_dead());
        assert_eq!(
            owner.get_body().unwrap().lock().unwrap().get_damage_state(),
            BodyDamageType::Rubble
        );
    }
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&pos),
        Some(crate::ai::pathfind_astar::PathfindCellType::Obstacle),
        "existing initial occupancy stamp uses the driving structure footprint"
    );
}

#[path = "w3d_owner_context_tests.rs"]
mod w3d_owner_context_tests;

#[path = "particle_callback_tests.rs"]
mod particle_callback_tests;
