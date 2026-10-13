//! Actual authored StructureBody installation and the real classic PF boundary.
//! ActiveBody.cpp:138-162,184-210: geometryZ -> PF remove -> PF add -> status.
//! Initial construction has no cached CPP body during PF insertion, unlike a
//! later damage callback. This packet preserves the existing initial obstacle
//! stamp; complete CELL_RUBBLE/obstacle-owner classification is separate debt.
use crate::ai::pathfind_astar::PathfindCellType;
use crate::common::{Coord3D, ObjectID, ObjectStatusMaskType, ThingTemplate};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::{Object, registry::OBJECT_REGISTRY};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::sync::{Arc, RwLock};

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_BODY_RUBBLE_CHILD",
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

// Retire only actual admissions made by this factory, including on assertion
// unwind. Keep unadmitted same-ID owners alive until that exact canonical
// lifetime has ended, so their standalone destructor cannot observe it.
struct Admissions {
    factory: ObjectFactory,
    unadmitted: Vec<Arc<RwLock<Object>>>,
}

impl Admissions {
    fn new() -> Self {
        Self {
            factory: ObjectFactory::new(),
            unadmitted: Vec::new(),
        }
    }
}

impl Drop for Admissions {
    fn drop(&mut self) {
        {
            let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
            self.factory.clear_all_objects(&mut logic).unwrap();
        }
        // Raw Object destruction uses its standalone adapters, so never
        // retain the canonical GameLogic guard across this release.
        self.unadmitted.clear();
    }
}

fn authored(name: &str, initial: u32) -> Arc<dyn ThingTemplate> {
    assert!(ensure_thing_factory_exists());
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
        "Object {name}\nKindOf = STRUCTURE IMMOBILE\nGeometry = SPHERE\nGeometryMajorRadius = 5\nGeometryMinorRadius = 5\nGeometryHeight = 20\nGeometryIsSmall = No\nStructureRubbleHeight = 3\nBody = StructureBody RubbleBody\nMaxHealth = 400\nInitialHealth = {initial}\nEnd\nEnd\n"
    )), 1);
    TheThingFactory::find_template(name).unwrap()
}

fn pathfinder() -> Arc<RwLock<crate::ai::Pathfinder>> {
    let mut terrain = crate::terrain::TerrainLogic::new();
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 16;
    map.height = 16;
    map.heightmap = vec![0; 256];
    map.boundaries = vec![crate::common::ICoord2D::new(16, 16)];
    terrain.load_map_data(map);
    let pf = crate::ai::the_ai().read().unwrap().pathfinder().unwrap();
    pf.write().unwrap().rebuild_from_terrain(&terrain);
    assert!(
        pf.read().unwrap().is_map_ready(),
        "actual loaded terrain initializes PF"
    );
    pf
}

fn raw(template: Arc<dyn ThingTemplate>, id: ObjectID, pos: Coord3D) -> Arc<RwLock<Object>> {
    let owner = Arc::new(RwLock::new(Object::new_raw(
        template,
        id,
        ObjectStatusMaskType::NONE,
        None,
    )));
    owner.write().unwrap().set_position(&pos).unwrap();
    owner
}

fn assert_rubble(owner: &Arc<RwLock<Object>>) {
    let owner = owner.read().unwrap();
    assert!(
        owner
            .get_status_bits()
            .contains(ObjectStatusMaskType::NO_COLLISIONS)
    );
    assert_eq!(
        owner.get_geometry_info().get_max_height_above_position(),
        3.0
    );
    let body = owner.get_body().expect("actual authored body cache");
    let body = body.lock().unwrap();
    assert_eq!(body.get_health(), 0.0);
    assert_eq!(
        body.get_damage_state(),
        crate::common::BodyDamageType::Rubble
    );
}

#[test]
fn initial_rubble_installed_same_id_owners_use_their_own_footprints() {
    if !child(concat!(
        module_path!(),
        "::initial_rubble_installed_same_id_owners_use_their_own_footprints"
    )) {
        return;
    }
    let pf = pathfinder();
    assert!(OBJECT_REGISTRY.is_empty());
    let first_template = authored("InitialRubbleFirst", 0);
    let second_template = authored("InitialRubbleSecond", 0);
    let id = 0x7B3D_0041;
    let first_pos = Coord3D::new(35.0, 35.0, 0.0);
    let second_pos = Coord3D::new(75.0, 75.0, 0.0);
    let first = raw(first_template.clone(), id, first_pos);
    let second = raw(second_template.clone(), id, second_pos);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&first_pos),
        Some(PathfindCellType::Clear)
    );
    Object::init_modules_for(&first, first_template.as_ref()).unwrap();
    assert_rubble(&first);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&first_pos),
        Some(PathfindCellType::Obstacle)
    );
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&second_pos),
        Some(PathfindCellType::Clear)
    );
    Object::init_modules_for(&second, second_template.as_ref()).unwrap();
    assert_rubble(&second);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&second_pos),
        Some(PathfindCellType::Obstacle)
    );
    assert!(!Arc::ptr_eq(
        &first.read().unwrap().get_body().unwrap(),
        &second.read().unwrap().get_body().unwrap()
    ));
    assert!(
        OBJECT_REGISTRY.is_empty(),
        "installation does not publish either same-ID owner"
    );
    drop(first);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&first_pos),
        Some(PathfindCellType::Clear),
        "destruction removes the driving owner's footprint without registry admission"
    );
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&second_pos),
        Some(PathfindCellType::Obstacle),
        "the other same-ID owner's footprint remains installed"
    );
    drop(second);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&second_pos),
        Some(PathfindCellType::Clear)
    );
}

#[test]
fn initial_rubble_ignores_foreign_same_id_admitted_owner_footprint() {
    if !child(concat!(
        module_path!(),
        "::initial_rubble_ignores_foreign_same_id_admitted_owner_footprint"
    )) {
        return;
    }
    let _ = authored("InitialRubbleForeign", 300);
    let mut admissions = Admissions::new();
    let foreign_pos = Coord3D::new(35.0, 35.0, 0.0);
    let id = admissions
        .factory
        .create_object(
            "InitialRubbleForeign",
            foreign_pos,
            None,
            ObjectCreationFlags::NO_AI | ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let foreign = admissions
        .factory
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(Arc::ptr_eq(
        &foreign,
        &TheGameLogic::find_object_by_id(id).unwrap()
    ));
    assert!(!OBJECT_REGISTRY.is_empty());
    let pf = pathfinder();
    let template = authored("InitialRubbleUnadmitted", 0);
    let driving_pos = Coord3D::new(75.0, 75.0, 0.0);
    let driving = raw(template.clone(), id, driving_pos);
    admissions.unadmitted.push(Arc::clone(&driving));
    Object::init_modules_for(&driving, template.as_ref()).unwrap();
    assert_rubble(&driving);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&driving_pos),
        Some(PathfindCellType::Obstacle)
    );
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&foreign_pos),
        Some(PathfindCellType::Clear),
        "foreign same-ID geometry is not the driving footprint"
    );
    assert_eq!(
        foreign
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        300.0
    );
    drop(driving);
    drop(admissions);
    assert!(TheGameLogic::find_object_by_id(id).is_none());
}

#[test]
fn admitted_initial_rubble_pose_returns_under_actual_owner_write_loan() {
    if !child(concat!(
        module_path!(),
        "::admitted_initial_rubble_pose_returns_under_actual_owner_write_loan"
    )) {
        return;
    }
    let _ = authored("InitialRubbleAdmitted", 0);
    let mut admissions = Admissions::new();
    let pos = Coord3D::new(35.0, 35.0, 0.0);
    let id = admissions
        .factory
        .create_object(
            "InitialRubbleAdmitted",
            pos,
            None,
            ObjectCreationFlags::NO_AI | ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let owner = admissions
        .factory
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(Arc::ptr_eq(
        &owner,
        &OBJECT_REGISTRY.get_object(id).unwrap()
    ));
    let pf = pathfinder();
    {
        let mut owner = owner.write().unwrap();
        owner.apply_structure_rubble_pose();
        assert!(
            owner
                .get_status_bits()
                .contains(ObjectStatusMaskType::NO_COLLISIONS)
        );
    }
    assert_rubble(&owner);
    assert_eq!(
        pf.read().unwrap().get_cell_type_at(&pos),
        Some(PathfindCellType::Obstacle)
    );
    drop(admissions);
    assert!(TheGameLogic::find_object_by_id(id).is_none());
}
