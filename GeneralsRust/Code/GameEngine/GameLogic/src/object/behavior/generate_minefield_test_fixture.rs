//! Real match-local factory inputs. No process registry, published world, or RNG restore.
use super::*;
use crate::object::body::active_body::BodyDefinitionCatalog;
use crate::object::collide::collision_system::CollisionSystem;
use crate::object::{
    Object,
    object_factory::{ObjectCreationFlags, ObjectFactory},
};
use crate::system::game_logic::GameLogic;
use game_engine::common::random_value::RandomState;
use game_engine::common::thing::{module_factory::ModuleFactory, thing_factory::ThingFactory};
use std::collections::HashMap;

pub(super) struct MinefieldFixture {
    pub id: u32,
    pub template: String,
    pub factory: ObjectFactory,
    scheduler: GameLogic,
    definitions: ThingFactory,
    modules: ModuleFactory,
    bodies: Arc<BodyDefinitionCatalog>,
    terrain: crate::terrain::TerrainLogic,
    collision: CollisionSystem,
    random: RandomState,
}
impl MinefieldFixture {
    pub fn new(id: u32) -> Self {
        let template = format!("OwnedMinefieldFixtureMine{id}");
        let mut definitions = ThingFactory::new();
        assert_eq!(definitions.load_ini_text(&format!("Object {template}\n KindOf = MINE\n Geometry = CYLINDER\n GeometryMajorRadius = 3.0\n GeometryMinorRadius = 3.0\n GeometryHeight = 1.0\nEnd\nObject OwnedMinefieldSource\n Geometry = CYLINDER\n GeometryMajorRadius = 20.0\n GeometryMinorRadius = 20.0\n GeometryHeight = 1.0\nEnd\nObject OwnedMinefieldStructure\n KindOf = STRUCTURE\n Geometry = CYLINDER\n GeometryMajorRadius = 4.0\n GeometryMinorRadius = 4.0\n GeometryHeight = 4.0\nEnd\n")), 3);
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![24; 64 * 64];
        map.boundaries = vec![crate::common::ICoord2D::new(64, 64)];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_geometry(map);
        let mut fixture = Self {
            id,
            template,
            factory: ObjectFactory::new(),
            scheduler: GameLogic::new(),
            definitions,
            modules: ModuleFactory::new(),
            bodies: Arc::new(
                BodyDefinitionCatalog::new(
                    Some(&game_engine::common::global_data::GlobalData::default()),
                    HashMap::new(),
                    HashMap::new(),
                )
                .unwrap(),
            ),
            terrain,
            collision: CollisionSystem::new(),
            random: RandomState::default(),
        };
        let template = fixture.definition("OwnedMinefieldSource");
        let mut lookup = |_name: &str| Ok(None);
        fixture
            .factory
            .prepare_saved_object(
                template,
                id,
                &mut lookup,
                fixture.bodies.clone(),
                &fixture.modules,
            )
            .unwrap();
        let owner = fixture
            .factory
            .get_object(id)
            .unwrap()
            .get_base_object()
            .unwrap();
        fixture
            .scheduler
            .track_object_in_update_list(owner)
            .unwrap();
        fixture.position(id, Coord3D::new(128.0, 128.0, 3.0), 0.0);
        fixture
    }
    fn definition(&self, name: &str) -> Arc<dyn crate::common::ThingTemplate> {
        crate::helpers::adapt_owned_thing_template(
            self.definitions
                .find_template(name, false)
                .expect("authored definition"),
        )
    }
    fn position(&mut self, id: u32, position: Coord3D, orientation: f32) {
        let owner = self
            .factory
            .get_object(id)
            .unwrap()
            .get_base_object()
            .unwrap();
        let mut owner = owner.write().unwrap();
        let bounds = owner.get_geometry_info().bounds();
        let radius = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y) * 0.5;
        self.collision
            .register_object(
                id,
                crate::object::collide::Coord3D::new(position.x, position.y, position.z),
                crate::object::collide::collision_geometry::GeometryInfo::new_cylinder(
                    radius, 1.0, false,
                ),
                None,
            )
            .unwrap();
        let matrix = glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_z(orientation),
            glam::Vec3::new(position.x, position.y, position.z),
        );
        owner
            .set_transform_matrix_with_services(
                &matrix,
                &self.terrain,
                &mut self.collision,
                0,
                &mut |_| {},
                &mut |_| {},
            )
            .unwrap();
    }
    fn spawn(&mut self, name: &str, position: Coord3D, orientation: f32) -> u32 {
        let template = self.definition(name);
        let mut lookup = |_name: &str| Ok(None);
        let id = self
            .factory
            .prepare_object_owned(
                &mut self.scheduler,
                template,
                Coord3D::ZERO,
                None,
                ObjectCreationFlags::NO_AI,
                &mut lookup,
                self.bodies.clone(),
                &self.modules,
            )
            .unwrap();
        self.position(id, position, orientation);
        id
    }
    pub fn behavior(&self, on_death: bool) -> GenerateMinefieldBehavior {
        GenerateMinefieldBehavior::new_from_config(
            self.id,
            GenerateMinefieldBehaviorModuleData {
                base: BehaviorModuleData::default(),
                mine_name: self.template.clone(),
                mine_name_upgraded: None,
                mine_upgrade_trigger: None,
                generation_fx: None,
                distance_around_object: 8.0,
                mines_per_square_foot: 0.01,
                random_jitter: 0.0,
                skip_if_this_much_under_structure: 0.33,
                on_death,
                border_only: true,
                always_circular: true,
                upgradable: true,
                smart_border: false,
                smart_border_skip_interior: true,
            },
        )
    }
    pub fn spawn_structure(&mut self, position: Coord3D) -> u32 {
        self.spawn("OwnedMinefieldStructure", position, 0.0)
    }
    pub fn assert_real_mines(&self, behavior: &GenerateMinefieldBehavior) -> Vec<u32> {
        let ids = behavior.get_mine_list();
        assert!(
            !ids.is_empty(),
            "generation must create actual factory mines"
        );
        let (geometry, radius) = self.preflight(self.id, &self.template).unwrap();
        let field_radius = geometry.major_radius + 8.0;
        let expected = (2.0 * std::f32::consts::PI * field_radius / (2.0 * radius)).ceil() as usize;
        assert_eq!(ids.len(), expected);
        for id in &ids {
            let owner = self
                .factory
                .get_object(*id)
                .unwrap()
                .get_base_object()
                .unwrap();
            assert!(Arc::ptr_eq(
                &owner,
                &self.scheduler.find_object_by_id(*id).unwrap()
            ));
            let mine = owner.read().unwrap();
            assert_eq!(mine.get_producer_id(), self.id);
            assert_eq!(mine.get_template().get_name().as_str(), self.template);
            assert!(mine.is_kind_of(crate::common::KindOf::Mine));
            let dx = mine.get_position().x - geometry.center.x;
            let dy = mine.get_position().y - geometry.center.y;
            assert!(((dx * dx + dy * dy).sqrt() - field_radius).abs() < 0.001);
        }
        ids
    }
}
impl MinefieldPlacement for MinefieldFixture {
    fn preflight(&self, owner: u32, template: &str) -> BehaviorResult<(GeometryInfo, f32)> {
        let actor = self
            .scheduler
            .find_object_by_id(owner)
            .ok_or(BehaviorError::ObjectNotFound { id: owner })?;
        let actor = actor
            .read()
            .map_err(|_| BehaviorError::ServiceUnavailable("owner poisoned".into()))?;
        let bounds = actor.get_geometry_info().bounds();
        let definition = self.definition(template);
        if !definition.get_behavior_module_info().is_empty()
            || !definition.get_draw_module_info().is_empty()
        {
            return Err(BehaviorError::ServiceUnavailable(
                "mine module callbacks require supplied scoot/arming services".into(),
            ));
        }
        Ok((
            GeometryInfo {
                center: *actor.get_position(),
                major_radius: (bounds.max.x - bounds.min.x) * 0.5,
                minor_radius: (bounds.max.y - bounds.min.y) * 0.5,
                rotation: actor.get_orientation(),
                is_circular: true,
            },
            definition
                .get_template_geometry_info()
                .get_bounding_circle_radius(),
        ))
    }
    fn random_real(&mut self, low: f32, high: f32) -> f32 {
        self.random.next_real(low, high)
    }
    fn place(
        &mut self,
        owner: u32,
        template: &str,
        position: &Coord3D,
        skip: f32,
    ) -> BehaviorResult<u32> {
        let (_, radius) = self.preflight(owner, template)?;
        let mut highest = *position;
        highest.z = 99999.0;
        if self.terrain.get_highest_layer_for_destination(&highest)
            == crate::path::PathfindLayerEnum::Ground
            && (self
                .terrain
                .is_underwater(position.x, position.y, None, None)
                || self.terrain.is_cliff_cell(position.x, position.y))
        {
            return Err(BehaviorError::InvalidPosition);
        }
        // C++ consumes orientation after terrain validity and before overlap rejection.
        let orientation = self
            .random
            .next_real(-std::f32::consts::PI, std::f32::consts::PI);
        let query = crate::object::collide::Coord3D::new(position.x, position.y, position.z);
        for id in self
            .collision
            .query_objects_in_radius(&query, radius * (1.0 - skip))
        {
            if self.scheduler.find_object_by_id(id).is_some_and(|actor| {
                actor
                    .read()
                    .unwrap()
                    .is_kind_of(crate::common::KindOf::Structure)
            }) {
                return Err(BehaviorError::NoSpaceAvailable);
            }
        }
        let id = self.spawn(template, *position, orientation);
        let source = self.scheduler.find_object_by_id(owner).unwrap();
        let mine = self.scheduler.find_object_by_id(id).unwrap();
        mine.write()
            .unwrap()
            .set_producer(Some(&*source.read().unwrap()));
        Ok(id)
    }
    fn remove(&mut self, owner: u32, mine: u32) -> BehaviorResult<()> {
        let actor = self
            .scheduler
            .find_object_by_id(mine)
            .ok_or(BehaviorError::ObjectNotFound { id: mine })?;
        let plan = {
            let mut object = actor.write().unwrap();
            if object.get_producer_id() != owner {
                return Err(BehaviorError::ServiceUnavailable(
                    "mine producer mismatch".into(),
                ));
            }
            let plan = object
                .native_destroy_plan()
                .map_err(BehaviorError::ServiceUnavailable)?;
            object.begin_native_destroy();
            plan
        };
        self.scheduler
            .queue_native_destroy(mine, &actor)
            .map_err(BehaviorError::ServiceUnavailable)?;
        actor.write().unwrap().mark_native_destroy_notified();
        plan.notify_delete()
            .map_err(BehaviorError::ServiceUnavailable)?;
        self.factory.track_destroyed_object(mine);
        Ok(())
    }
}

#[test]
fn actual_structure_rejection_and_factory_creation_are_match_local() {
    let mut fixture = MinefieldFixture::new(98_104);
    let blocked = Coord3D::new(192.0, 192.0, 3.0);
    let free = Coord3D::new(224.0, 192.0, 3.0);
    let obstruction = fixture.spawn_structure(blocked);
    assert!(
        fixture
            .scheduler
            .find_object_by_id(obstruction)
            .unwrap()
            .read()
            .unwrap()
            .is_kind_of(crate::common::KindOf::Structure)
    );
    let before = fixture.scheduler.get_all_object_ids().to_vec();
    let template = fixture.template.clone();
    assert!(matches!(
        fixture.place(fixture.id, &template, &blocked, 0.33),
        Err(BehaviorError::NoSpaceAvailable)
    ));
    assert_eq!(fixture.scheduler.get_all_object_ids(), before);
    let id = fixture.place(fixture.id, &template, &free, 0.33).unwrap();
    let mine = fixture.scheduler.find_object_by_id(id).unwrap();
    let mine = mine.read().unwrap();
    assert_eq!(mine.get_producer_id(), fixture.id);
    assert_eq!(*mine.get_position(), free);
    assert!(mine.is_kind_of(crate::common::KindOf::Mine));
    assert_eq!(mine.get_template().get_name().as_str(), template);
    assert_eq!(
        fixture.scheduler.get_all_object_ids().len(),
        before.len() + 1
    );
}

#[test]
fn same_id_minefields_create_and_clear_only_the_supplied_match() {
    let mut first = MinefieldFixture::new(98_105);
    let mut second = MinefieldFixture::new(98_105);
    let mut first_behavior = first.behavior(false);
    let second_behavior = second.behavior(false);
    first_behavior.place_mines_owned(&mut first).unwrap();
    let ids = first.assert_real_mines(&first_behavior);
    assert!(!second_behavior.get_statistics().is_generated);
    assert_eq!(second.scheduler.get_all_object_ids(), &[second.id]);
    // A constructor and an operation on the peer cannot redirect first's owner.
    let mut peer = second.behavior(false);
    peer.place_mines_owned(&mut second).unwrap();
    let peer_ids = second.assert_real_mines(&peer);
    first_behavior.clear_mines_owned(&mut first).unwrap();
    for id in ids {
        assert!(
            first
                .scheduler
                .find_object_by_id(id)
                .unwrap()
                .read()
                .unwrap()
                .is_destroyed()
        );
    }
    for id in peer_ids {
        assert!(
            !second
                .scheduler
                .find_object_by_id(id)
                .unwrap()
                .read()
                .unwrap()
                .is_destroyed()
        );
    }
    assert!(peer.get_statistics().is_generated);
}

#[test]
fn unsupported_generation_fx_rejects_before_creation_or_generation() {
    let mut fixture = MinefieldFixture::new(98_106);
    let mut behavior = fixture.behavior(false);
    behavior.config.generation_fx = Some("AuthoredMineFX".into());
    let before = fixture.scheduler.get_all_object_ids().to_vec();
    assert!(matches!(
        behavior.place_mines_owned(&mut fixture),
        Err(BehaviorError::ServiceUnavailable(_))
    ));
    assert_eq!(fixture.scheduler.get_all_object_ids(), before);
    assert!(!behavior.get_statistics().is_generated);
    assert_eq!(behavior.get_statistics().mine_count, 0);
}
