//! Real minefield inputs and exact authoritative fixture retirement.

use super::*;
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::system::game_logic::get_game_logic;
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, RwLock};

pub(super) struct MinefieldFixture {
    pub id: u32,
    pub template: String,
    object: Arc<RwLock<Object>>,
    added_objects: Vec<(u32, Arc<RwLock<Object>>)>,
    terrain: Option<crate::terrain::TerrainLogic>,
    seed: [u32; 6],
}

impl MinefieldFixture {
    pub fn new(id: u32) -> Self {
        let template = format!("OwnedMinefieldFixtureMine{id}");
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let loaded = {
            let mut factory = get_thing_factory().unwrap();
            let factory = factory.as_mut().unwrap();
            if factory.find_template(&template, false).is_none() {
                let rules = format!(
                    "Object {template}\n  KindOf = MINE\n  Geometry = CYLINDER\n  GeometryMajorRadius = 3.0\n  GeometryMinorRadius = 3.0\n  GeometryHeight = 1.0\nEnd\n"
                );
                Some(factory.load_ini_text(&rules))
            } else {
                None
            }
        };
        if let Some(loaded) = loaded {
            assert_eq!(loaded, 1);
        }
        // The real terrain rejection checks run against an authored flat map.
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![24; 64 * 64];
        map.boundaries = vec![crate::common::ICoord2D::new(64, 64)];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_data(map);
        let previous = std::mem::replace(
            &mut *crate::terrain::get_terrain_logic().write().unwrap(),
            terrain,
        );
        let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        let positioned = object
            .write()
            .unwrap()
            .set_position(&Coord3D::new(128.0, 128.0, 3.0));
        positioned.expect("minefield owner position");
        let fixture = Self {
            id,
            template,
            object,
            added_objects: Vec::new(),
            terrain: Some(previous),
            seed: game_engine::common::random_value::get_game_logic_random_seed_state(),
        };
        assert!(OBJECT_REGISTRY.get_object(id).is_none());
        let admission = get_game_logic()
            .lock()
            .unwrap()
            .register_object(fixture.object.clone());
        admission.expect("canonical fixture admission");
        fixture
    }

    pub fn behavior(&self, on_death: bool) -> GenerateMinefieldBehavior {
        GenerateMinefieldBehaviorBuilder::new()
            .mine_name(&self.template)
            .distance_around_object(8.0)
            .border_only(true)
            .always_circular(true)
            .upgradable(true)
            .random_jitter(0.0)
            .on_death(on_death)
            .build(self.id)
    }

    /// Create a genuine authored obstruction and retain its exact retirement identity.
    pub fn spawn_structure(&mut self, position: Coord3D) -> u32 {
        let template_name = format!("OwnedMinefieldFixtureStructure{}", self.id);
        let loaded = {
            let mut factory = get_thing_factory().unwrap();
            let factory = factory.as_mut().unwrap();
            if factory.find_template(&template_name, false).is_none() {
                let rules = format!(
                    "Object {template_name}\n  KindOf = STRUCTURE\n  Geometry = CYLINDER\n  GeometryMajorRadius = 4.0\n  GeometryMinorRadius = 4.0\n  GeometryHeight = 4.0\nEnd\n"
                );
                Some(factory.load_ini_text(&rules))
            } else {
                None
            }
        };
        if let Some(loaded) = loaded {
            assert_eq!(loaded, 1);
        }
        let template = crate::helpers::TheThingFactory::find_template(&template_name)
            .expect("authored structure template");
        let object = crate::helpers::TheThingFactory::get()
            .unwrap()
            .new_object_optional_team(template, None)
            .expect("actual factory structure");
        let id = object.read().unwrap().get_id();
        self.added_objects.push((id, Arc::clone(&object)));
        let positioned = object.write().unwrap().set_position(&position);
        positioned.expect("actual structure position");
        // The factory registers its initial pose. set_position currently leaves
        // this canonical cache stale (hq-0jgag), so use the same registration
        // boundary with the actual new pose, never a fabricated candidate list.
        crate::helpers::ThePartitionManager::get()
            .unwrap()
            .register_object_at(id, position);
        id
    }

    pub fn assert_real_mines(&self, behavior: &GenerateMinefieldBehavior) -> Vec<u32> {
        let ids = behavior.get_mine_list();
        assert!(
            !ids.is_empty(),
            "generation must create actual factory mines"
        );
        let radius = crate::helpers::TheThingFactory::find_template(&self.template)
            .unwrap()
            .get_template_geometry_info()
            .get_bounding_circle_radius();
        let geometry = behavior.get_object_geometry().unwrap();
        let field_radius = geometry.major_radius + 8.0;
        let expected = (2.0 * std::f32::consts::PI * field_radius / (2.0 * radius)).ceil() as usize;
        assert_eq!(ids.len(), expected);
        let facts: Result<Vec<_>, String> = (|| {
            let logic = get_game_logic()
                .lock()
                .map_err(|_| "minefield canonical owner poisoned".to_string())?;
            ids.iter()
                .map(|id| {
                    let mine = logic
                        .find_object_by_id(*id)
                        .ok_or_else(|| format!("mine {id} missing canonical admission"))?;
                    let mine = mine.read().map_err(|_| format!("mine {id} poisoned"))?;
                    Ok((
                        mine.get_producer_id(),
                        mine.get_template().get_name().as_str().to_owned(),
                        mine.is_kind_of(crate::common::KindOf::Mine),
                        *mine.get_position(),
                    ))
                })
                .collect()
        })();
        // Validation failures must never unwind through gameplay guards.
        for (producer, template, is_mine, pos) in facts.expect("actual canonical mine facts") {
            assert_eq!(producer, self.id);
            assert_eq!(template, self.template);
            assert!(is_mine);
            let dx = pos.x - geometry.center.x;
            let dy = pos.y - geometry.center.y;
            assert!(((dx * dx + dy * dy).sqrt() - field_radius).abs() < 0.001);
        }
        ids
    }

    fn retire(&self) -> Result<(), String> {
        let mut logic = get_game_logic()
            .lock()
            .map_err(|_| "minefield canonical owner poisoned".to_string())?;
        let admitted = logic
            .find_object_by_id(self.id)
            .ok_or_else(|| format!("fixture {} missing canonical admission", self.id))?;
        if !Arc::ptr_eq(&admitted, &self.object) {
            return Err(format!("fixture {} canonical identity mismatch", self.id));
        }
        let mut mines = Vec::new();
        for object in logic.iter_all_objects() {
            let mine = object
                .read()
                .map_err(|_| "minefield candidate object poisoned".to_string())?;
            if mine.get_producer_id() == self.id {
                mines.push((mine.get_id(), object.clone()));
            }
        }
        for (id, expected) in &self.added_objects {
            let admitted = logic
                .find_object_by_id(*id)
                .ok_or_else(|| format!("added fixture {id} missing canonical admission"))?;
            if !Arc::ptr_eq(&admitted, expected) {
                return Err(format!("added fixture {id} canonical identity mismatch"));
            }
            if !mines.iter().any(|(candidate, _)| candidate == id) {
                mines.push((*id, Arc::clone(expected)));
            }
        }
        let detached: Vec<_> = {
            let manager_handle = get_object_manager();
            let mut manager = manager_handle
                .write()
                .map_err(|_| "minefield object manager poisoned".to_string())?;
            mines
                .iter()
                .map(|(id, object)| {
                    manager
                        .detach_fixture_object_slot(*id, object, &logic)
                        .map(|slot| (*id, slot, object.clone()))
                })
                .collect::<Result<_, _>>()?
        };
        // Pins remain alive; neither manager nor object guard spans callbacks.
        for (id, _) in &mines {
            logic.destroy_object(*id);
        }
        logic.destroy_object(self.id);
        logic
            .cleanup_dead_objects()
            .map_err(|error| format!("fixture cleanup: {error:?}"))?;
        {
            let manager_handle = get_object_manager();
            let manager = manager_handle
                .read()
                .map_err(|_| "minefield object manager poisoned".to_string())?;
            for (id, slot, expected) in detached {
                manager.finish_fixture_object_slot_retirement(id, slot, &expected, &logic)?;
            }
        }
        if logic.find_object_by_id(self.id).is_some() {
            return Err(format!("fixture {} remains canonically admitted", self.id));
        }
        if OBJECT_REGISTRY.get_object(self.id).is_some() {
            return Err(format!("fixture {} remains discoverable", self.id));
        }
        Ok(())
    }
}

impl Drop for MinefieldFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let result = self.retire();
            result.expect("exact minefield fixture retirement");
        }));
        *crate::terrain::get_terrain_logic().write().unwrap() = self.terrain.take().unwrap();
        crate::helpers::set_game_logic_random_seed(self.seed);
        if let Err(error) = result {
            if unwinding {
                eprintln!("minefield fixture retirement failed during unwind");
            } else {
                resume_unwind(error);
            }
        }
    }
}

#[test]
fn structure_rejection_consumes_orientation_before_next_real_mine() {
    use game_engine::common::random_value::{
        get_game_logic_random_seed_state, set_game_logic_random_seed_state,
    };

    let _guard = crate::test_sync::lock();
    let mut fixture = MinefieldFixture::new(98_104);
    let behavior = fixture.behavior(false);
    let blocked = Coord3D::new(192.0, 192.0, 3.0);
    let free = Coord3D::new(224.0, 192.0, 3.0);
    let structure_id = fixture.spawn_structure(blocked);
    let structure_kind = crate::object::registry::OBJECT_REGISTRY
        .with_object(structure_id, |object| {
            object.is_kind_of(crate::common::KindOf::Structure)
        });
    assert_eq!(structure_kind, Some(true));
    let query_radius = behavior.mine_template_radius(&fixture.template).unwrap()
        * (1.0 - behavior.config.skip_if_this_much_under_structure);
    let partition = crate::helpers::ThePartitionManager::get().unwrap();
    assert!(
        partition
            .get_objects_in_range(&blocked, query_radius)
            .contains(&structure_id)
    );
    assert!(
        !partition
            .get_objects_in_range(&free, query_radius)
            .contains(&structure_id)
    );

    // Read canonical facts under guards, but validate outside them. Exact
    // IDs also prove rejection did not fabricate or create a mine.
    let canonical_ids = || {
        let ids = crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .get_all_object_ids()
            .to_vec();
        let mut ids = ids;
        ids.sort_unstable();
        ids
    };
    let before_ids = canonical_ids();
    let before_seed = [0x1234, 0x5678, 0x9abc, 0xdef0, 0x1357, 0x2468];
    set_game_logic_random_seed_state(before_seed);
    let _rejected_orientation = behavior.random_value(-std::f32::consts::PI, std::f32::consts::PI);
    let after_one_draw = get_game_logic_random_seed_state();
    let expected_orientation = behavior.random_value(-std::f32::consts::PI, std::f32::consts::PI);
    let after_two_draws = get_game_logic_random_seed_state();
    set_game_logic_random_seed_state(before_seed);

    // CPP GenerateMinefieldBehavior.cpp:170-197: valid terrain -> draw
    // orientation -> real structure overlap -> rejection without creation.
    let rejected = behavior.place_mine_at(&blocked, &fixture.template);
    assert!(matches!(rejected, Err(BehaviorError::NoSpaceAvailable)));
    assert_eq!(canonical_ids(), before_ids);
    assert_eq!(get_game_logic_random_seed_state(), after_one_draw);

    // CPP:199-212 creates a real mine using the following orientation.
    // Authored templates have no build variations or random module hooks.
    let mine_id = behavior
        .place_mine_at(&free, &fixture.template)
        .expect("unblocked actual factory mine");
    let facts = crate::object::registry::OBJECT_REGISTRY
        .with_object(mine_id, |mine| {
            (
                mine.get_producer_id(),
                mine.get_template().get_name().as_str().to_owned(),
                mine.is_kind_of(crate::common::KindOf::Mine),
                *mine.get_position(),
                mine.get_orientation(),
            )
        })
        .expect("actual created mine");
    assert_eq!(facts.0, fixture.id);
    assert_eq!(facts.1, fixture.template);
    assert!(facts.2);
    assert_eq!(facts.3, free);
    assert_eq!(facts.4, expected_orientation);
    assert_eq!(get_game_logic_random_seed_state(), after_two_draws);
    let mut expected_ids = before_ids;
    expected_ids.push(mine_id);
    expected_ids.sort_unstable();
    assert_eq!(canonical_ids(), expected_ids);
}
