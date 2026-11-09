//! Instance-owned GenerateMinefieldBehavior placement services.
//! C++ GenerateMinefieldBehavior.cpp:354-475 calls ThingFactory::newObject,
//! GameLogic::destroyObject, TerrainLogic, PartitionManager and
//! GameLogicRandomValueReal during the module update. The match lends those
//! services to the scheduler; GameLogic owns none of them.
use super::*;
use crate::object::behavior::generate_minefield_behavior::{
    mine_structure_query_geometry, mine_would_collide,
};
use crate::object::body::active_body::BodyDefinitionCatalog;
use crate::object::collide::collision_system::CollisionSystem;
use crate::object::object_factory::ObjectFactory;
use game_engine::common::random_value::RandomState;
use game_engine::common::thing::update_module::{
    MinefieldFootprint, MinefieldServiceError, OwnedMinefieldServices,
};
use game_engine::common::thing::{module_factory::ModuleFactory, thing_factory::ThingFactory};

type ObjectMap = HashMap<ObjectID, Arc<RwLock<Object>>>;

/// Objects visible to a placement call: admitted objects plus mines born
/// earlier in this same update (C++ newObject admits immediately).
pub struct MinefieldView<'a> {
    objects: &'a ObjectMap,
    born: &'a [(ObjectID, Arc<RwLock<Object>>)],
}

impl MinefieldView<'_> {
    pub fn find(&self, id: ObjectID) -> Option<&Arc<RwLock<Object>>> {
        self.objects
            .get(&id)
            .or_else(|| self.born.iter().find(|(b, _)| *b == id).map(|(_, o)| o))
    }
    pub fn iter(&self) -> impl Iterator<Item = &Arc<RwLock<Object>>> {
        self.objects.values().chain(self.born.iter().map(|(_, o)| o))
    }
}

/// Borrowed match services for the minefield route. Mirrors
/// UpgradeCompletionServices: construction/validation run mid-update against a
/// read view; admission/destroyObject run with `&mut GameLogic` after the
/// module guard is released and before the queue resumes.
pub trait MinefieldPlacementServices {
    fn preflight(
        &self,
        view: &MinefieldView<'_>,
        owner: ObjectID,
        template: &str,
    ) -> Result<MinefieldFootprint, MinefieldServiceError>;
    fn random_real(&mut self, low: f32, high: f32) -> f32;
    /// Validate and construct a mine under the already-allocated `id`.
    fn construct(
        &mut self,
        view: &MinefieldView<'_>,
        id: ObjectID,
        owner: ObjectID,
        template: &str,
        position: Coord3D,
        skip: f32,
    ) -> Result<Arc<RwLock<Object>>, MinefieldServiceError>;
    /// C++ TheGameLogic->destroyObject for a mine this owner produced.
    fn destroy(
        &mut self,
        logic: &mut GameLogic,
        mine: ObjectID,
        actor: &Arc<RwLock<Object>>,
    ) -> Result<(), MinefieldServiceError>;
}

pub struct OwnedMinefieldPlacementServices<'a> {
    pub factory: &'a mut ObjectFactory,
    pub definitions: &'a ThingFactory,
    pub modules: &'a ModuleFactory,
    pub bodies: Arc<BodyDefinitionCatalog>,
    pub terrain: &'a crate::terrain::TerrainLogic,
    pub collision: &'a mut CollisionSystem,
    pub random: &'a mut RandomState,
}

fn unavailable(reason: impl Into<String>) -> MinefieldServiceError {
    MinefieldServiceError::Unavailable(reason.into())
}

impl OwnedMinefieldPlacementServices<'_> {
    fn definition(
        &self,
        name: &str,
    ) -> Result<Arc<dyn crate::common::ThingTemplate>, MinefieldServiceError> {
        let found = self
            .definitions
            .find_template(name, false)
            .ok_or_else(|| unavailable(format!("mine template {name} absent")))?;
        Ok(crate::helpers::adapt_owned_thing_template(found))
    }

    /// Partition registration + transform (C++ newObject then setPosition/setOrientation).
    fn position(
        &mut self,
        id: ObjectID,
        object: &Arc<RwLock<Object>>,
        position: Coord3D,
        orientation: f32,
    ) -> Result<(), MinefieldServiceError> {
        let mut owner = object.write().map_err(|_| unavailable("mine poisoned"))?;
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
            .map_err(|e| unavailable(format!("{e:?}")))?;
        let matrix = glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_z(orientation),
            glam::Vec3::new(position.x, position.y, position.z),
        );
        owner
            .set_transform_matrix_with_services(
                &matrix,
                self.terrain,
                self.collision,
                0,
                &mut |_| {},
                &mut |_| {},
            )
            .map_err(|e| unavailable(format!("{e:?}")))
    }
}

impl MinefieldPlacementServices for OwnedMinefieldPlacementServices<'_> {
    fn preflight(
        &self,
        view: &MinefieldView<'_>,
        owner: ObjectID,
        template: &str,
    ) -> Result<MinefieldFootprint, MinefieldServiceError> {
        let actor = view
            .find(owner)
            .ok_or(MinefieldServiceError::ObjectNotFound(owner))?;
        let actor = actor.read().map_err(|_| unavailable("owner poisoned"))?;
        let bounds = actor.get_geometry_info().bounds();
        let definition = self.definition(template)?;
        if !definition.get_behavior_module_info().is_empty()
            || !definition.get_draw_module_info().is_empty()
        {
            return Err(unavailable(
                "mine module callbacks require supplied scoot/arming services",
            ));
        }
        let center = actor.get_position();
        Ok(MinefieldFootprint {
            center: [center.x, center.y, center.z],
            major_radius: (bounds.max.x - bounds.min.x) * 0.5,
            minor_radius: (bounds.max.y - bounds.min.y) * 0.5,
            rotation: actor.get_orientation(),
            is_circular: true,
            mine_radius: definition
                .get_template_geometry_info()
                .get_bounding_circle_radius(),
        })
    }

    fn random_real(&mut self, low: f32, high: f32) -> f32 {
        self.random.next_real(low, high)
    }

    fn construct(
        &mut self,
        view: &MinefieldView<'_>,
        id: ObjectID,
        owner: ObjectID,
        template: &str,
        position: Coord3D,
        skip: f32,
    ) -> Result<Arc<RwLock<Object>>, MinefieldServiceError> {
        self.preflight(view, owner, template)?;
        let mut highest = position;
        highest.z = 99999.0;
        // GenerateMinefieldBehavior.cpp:160-175 terrain/layer validity.
        if self.terrain.get_highest_layer_for_destination(&highest)
            == crate::path::PathfindLayerEnum::Ground
            && (self
                .terrain
                .is_underwater(position.x, position.y, None, None)
                || self.terrain.is_cliff_cell(position.x, position.y))
        {
            return Err(MinefieldServiceError::InvalidPosition);
        }
        // Orientation draw precedes overlap rejection (C++ order).
        let orientation = self
            .random
            .next_real(-std::f32::consts::PI, std::f32::consts::PI);
        let definition = self.definition(template)?;
        let query = mine_structure_query_geometry(&definition.get_template_geometry_info(), skip);
        let max_dist = query.get_bounding_sphere_radius() * 1.1;
        for actor in view.iter() {
            let them = actor.read().map_err(|_| unavailable("object poisoned"))?;
            let geometry = them.get_geometry_info();
            let other = them.get_position();
            let center_z = (geometry.bounds().min.z + geometry.bounds().max.z) * 0.5;
            let (dx, dy, dz) = (
                other.x - position.x,
                other.y - position.y,
                other.z + center_z - position.z,
            );
            let boundary = ((dx * dx + dy * dy + dz * dz).sqrt()
                - geometry.get_bounding_sphere_radius())
            .max(0.0);
            if boundary <= max_dist
                && mine_would_collide(
                    &position,
                    &query,
                    orientation,
                    other,
                    &geometry,
                    them.get_orientation(),
                )
                && them.is_kind_of(crate::common::KindOf::Structure)
            {
                return Err(MinefieldServiceError::NoSpaceAvailable);
            }
        }
        // C++ ThingFactory::newObject under the identity GameLogic allocated.
        let mut lookup = |_name: &str| Ok(None);
        self.factory
            .prepare_saved_object(definition, id, &mut lookup, self.bodies.clone(), self.modules)
            .map_err(|e| unavailable(e.to_string()))?;
        let mine = self
            .factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .ok_or_else(|| unavailable("constructed mine absent"))?;
        self.position(id, &mine, position, orientation)?;
        let source = view
            .find(owner)
            .ok_or(MinefieldServiceError::ObjectNotFound(owner))?;
        mine.write()
            .map_err(|_| unavailable("mine poisoned"))?
            .set_producer(Some(&*source.read().map_err(|_| unavailable("owner poisoned"))?));
        Ok(mine)
    }

    fn destroy(
        &mut self,
        logic: &mut GameLogic,
        mine: ObjectID,
        actor: &Arc<RwLock<Object>>,
    ) -> Result<(), MinefieldServiceError> {
        let plan = {
            let mut object = actor.write().map_err(|_| unavailable("mine poisoned"))?;
            let plan = object.native_destroy_plan().map_err(unavailable)?;
            object.begin_native_destroy();
            plan
        };
        logic
            .queue_native_destroy(mine, actor)
            .map_err(unavailable)?;
        actor
            .write()
            .map_err(|_| unavailable("mine poisoned"))?
            .mark_native_destroy_notified();
        plan.notify_delete().map_err(unavailable)?;
        self.factory.track_destroyed_object(mine);
        Ok(())
    }
}

/// Mid-update effect awaiting the post-guard apply window of the same update.
pub(super) enum MinefieldEffect {
    Birth(ObjectID, Arc<RwLock<Object>>),
    Destroy(ObjectID, Arc<RwLock<Object>>),
}

/// The Common `OwnedMinefieldServices` hook as seen by a running module.
/// Place allocates the GameLogic ObjectID immediately (C++ newObject returns
/// it during update); admission/destroyObject apply after the guard drops.
pub(super) struct MinefieldWindow<'a> {
    pub services: &'a mut dyn MinefieldPlacementServices,
    pub objects: &'a ObjectMap,
    pub next_object_id: &'a mut ObjectID,
    pub effects: &'a mut Vec<MinefieldEffect>,
    pub born: Vec<(ObjectID, Arc<RwLock<Object>>)>,
}

impl MinefieldWindow<'_> {
    fn allocate(&mut self) -> ObjectID {
        // Same rule as GameLogic::allocate_object_id, also skipping same-update births.
        loop {
            let id = *self.next_object_id;
            *self.next_object_id = id.wrapping_add(1).max(1);
            if id != INVALID_ID
                && !self.objects.contains_key(&id)
                && !self.born.iter().any(|(b, _)| *b == id)
            {
                return id;
            }
        }
    }
}

impl OwnedMinefieldServices for MinefieldWindow<'_> {
    fn preflight(
        &self,
        owner: u32,
        template: &str,
    ) -> Result<MinefieldFootprint, MinefieldServiceError> {
        let view = MinefieldView { objects: self.objects, born: &self.born };
        self.services.preflight(&view, owner, template)
    }
    fn random_real(&mut self, low: f32, high: f32) -> f32 {
        self.services.random_real(low, high)
    }
    fn place(
        &mut self,
        owner: u32,
        template: &str,
        position: [f32; 3],
        skip: f32,
    ) -> Result<u32, MinefieldServiceError> {
        let [x, y, z] = position;
        // Validation precedes allocation: a rejected spot consumes no identity.
        let next = *self.next_object_id;
        let id = self.allocate();
        let view = MinefieldView { objects: self.objects, born: &self.born };
        match self
            .services
            .construct(&view, id, owner, template, Coord3D::new(x, y, z), skip)
        {
            Ok(mine) => {
                self.born.push((id, Arc::clone(&mine)));
                self.effects.push(MinefieldEffect::Birth(id, mine));
                Ok(id)
            }
            Err(error) => {
                *self.next_object_id = next;
                Err(error)
            }
        }
    }
    fn remove(&mut self, owner: u32, mine: u32) -> Result<(), MinefieldServiceError> {
        let view = MinefieldView { objects: self.objects, born: &self.born };
        let actor = view
            .find(mine)
            .cloned()
            .ok_or(MinefieldServiceError::ObjectNotFound(mine))?;
        if actor
            .read()
            .map_err(|_| unavailable("mine poisoned"))?
            .get_producer_id()
            != owner
        {
            return Err(unavailable("mine producer mismatch"));
        }
        self.effects.push(MinefieldEffect::Destroy(mine, actor));
        Ok(())
    }
    fn owner_completed_upgrades(&self, owner: u32) -> Option<u128> {
        let view = MinefieldView { objects: self.objects, born: &self.born };
        let owner = view.find(owner)?.read().ok()?;
        Some(owner.completed_upgrades().bits())
    }
}

impl GameLogic {
    /// Same-update apply window: after the module guard, before queue resume.
    pub(super) fn apply_minefield_effects(
        &mut self,
        effects: &mut Vec<MinefieldEffect>,
        services: &mut dyn MinefieldPlacementServices,
    ) {
        for effect in effects.drain(..) {
            match effect {
                MinefieldEffect::Birth(id, mine) => {
                    if let Err(error) = self.track_object_in_update_list(mine) {
                        warn!("minefield birth {id} admission failed: {error:?}");
                    }
                }
                MinefieldEffect::Destroy(id, actor) => {
                    if let Err(error) = services.destroy(self, id, &actor) {
                        warn!("minefield destroy {id} failed: {error:?}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::behavior::generate_minefield_behavior::{
        GenerateMinefieldBehavior, GenerateMinefieldBehaviorModuleData, ScheduledMinefield,
    };
    use crate::object::object_factory::ObjectCreationFlags;
    use game_engine::common::thing::update_module::{
        UpdateModuleInterface, UpdateScheduleContext,
    };

    /// Test-only route standing in for the .72 proxy: the installed module
    /// reaches the Common schedule hook during the real scheduler update.
    struct ScheduledMinefieldModule {
        behavior: GenerateMinefieldBehavior,
        upgrade: bool,
        mid_update_ids: Arc<std::sync::Mutex<Vec<ObjectID>>>,
    }

    impl UpdateModuleInterface for ScheduledMinefieldModule {
        fn update_scheduled(
            &mut self,
            context: &mut dyn UpdateScheduleContext,
        ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
            let services = context
                .owned_minefield_services()
                .ok_or("scheduler lent no minefield services")?;
            let mut placement = ScheduledMinefield(services);
            if self.upgrade {
                self.behavior
                    .update_upgrade_owned(true, &mut placement)
                    .map_err(|e| format!("{e:?}"))?;
            } else if self.behavior.get_mine_list().is_empty() {
                self.behavior
                    .place_mines_owned(&mut placement)
                    .map_err(|e| format!("{e:?}"))?;
            }
            // C++ newObject returned these IDs inside this update.
            *self.mid_update_ids.lock().unwrap() = self.behavior.get_mine_list();
            Ok(UpdateSleepTime::None)
        }
    }

    /// hq-2qedt.71: the real scheduler lends MinefieldPlacementServices; mines get
    /// GameLogic IDs mid-update, are admitted and destroyed in the same update.
    #[test]
    fn scheduler_minefield_services_place_and_destroy_real_mines_mid_update() {
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
            crate::system::engine_stores::with_active_stores(&stores, body)
        });
    }

    fn body() {
        let mut definitions = ThingFactory::new();
        assert_eq!(definitions.load_ini_text("Object SchedMine\n KindOf = MINE\n Geometry = CYLINDER\n GeometryMajorRadius = 3.0\n GeometryMinorRadius = 3.0\n GeometryHeight = 1.0\nEnd\nObject SchedEmpMine\n KindOf = MINE\n Geometry = CYLINDER\n GeometryMajorRadius = 3.0\n GeometryMinorRadius = 3.0\n GeometryHeight = 1.0\nEnd\nObject SchedMinefieldSource\n Geometry = CYLINDER\n GeometryMajorRadius = 20.0\n GeometryMinorRadius = 20.0\n GeometryHeight = 1.0\nEnd\n"), 3);
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![24; 64 * 64];
        map.boundaries = vec![crate::common::ICoord2D::new(64, 64)];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_geometry(map);
        let mut factory = ObjectFactory::new();
        let modules = ModuleFactory::new();
        let bodies = Arc::new(
            BodyDefinitionCatalog::new(
                Some(&game_engine::common::global_data::GlobalData::default()),
                HashMap::new(),
                HashMap::new(),
            )
            .unwrap(),
        );
        let mut collision = CollisionSystem::new();
        let mut random = RandomState::default();
        let mut logic = GameLogic::new();

        let source = crate::helpers::adapt_owned_thing_template(
            definitions.find_template("SchedMinefieldSource", false).unwrap(),
        );
        let mut lookup = |_name: &str| Ok(None);
        let owner_id = factory
            .prepare_object_owned(
                &mut logic,
                source,
                Coord3D::ZERO,
                None,
                ObjectCreationFlags::NO_AI,
                &mut lookup,
                bodies.clone(),
                &modules,
            )
            .unwrap();
        let mut services = OwnedMinefieldPlacementServices {
            factory: &mut factory,
            definitions: &definitions,
            modules: &modules,
            bodies: bodies.clone(),
            terrain: &terrain,
            collision: &mut collision,
            random: &mut random,
        };
        let owner = logic.find_object_by_id(owner_id).unwrap();
        services
            .position(owner_id, &owner, Coord3D::new(128.0, 128.0, 3.0), 0.0)
            .unwrap();

        let behavior = GenerateMinefieldBehavior::new_from_config(
            owner_id,
            GenerateMinefieldBehaviorModuleData {
                mine_name: "SchedMine".into(),
                mine_name_upgraded: Some("SchedEmpMine".into()),
                distance_around_object: 8.0,
                mines_per_square_foot: 0.01,
                random_jitter: 0.0,
                skip_if_this_much_under_structure: 0.33,
                border_only: true,
                always_circular: true,
                upgradable: true,
                ..GenerateMinefieldBehaviorModuleData::default()
            },
        );
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let module = Arc::new(RwLock::new(ScheduledMinefieldModule {
            behavior,
            upgrade: false,
            mid_update_ids: Arc::clone(&seen),
        }));
        let handle: UpdateModulePtr = module.clone();
        logic.register_normal_update_module(owner_id, handle);

        let counter = logic.get_object_id_counter();
        logic.frame = 5;
        logic.process_normal_updates_with_upgrade_services(None, Some(&mut services));

        let placed = seen.lock().unwrap().clone();
        assert!(!placed.is_empty(), "scheduler update placed no mines");
        // IDs come from this GameLogic's allocator, consecutively, mid-update.
        let expected: Vec<ObjectID> = (counter..counter + placed.len() as ObjectID).collect();
        assert_eq!(placed, expected);
        assert_eq!(logic.get_object_id_counter(), counter + placed.len() as ObjectID);
        for id in &placed {
            let mine = logic.find_object_by_id(*id).expect("mine admitted same update");
            assert!(logic.get_all_object_ids().contains(id));
            let factory_mine = services.factory.get_object(*id).unwrap().get_base_object().unwrap();
            assert!(Arc::ptr_eq(&mine, &factory_mine));
            let mine = mine.read().unwrap();
            assert_eq!(mine.get_producer_id(), owner_id);
            assert_eq!(mine.get_template().get_name().as_str(), "SchedMine");
        }

        // Upgrade tick: old mines go through destroyObject in the same update,
        // replacements get fresh IDs.
        module.write().unwrap().upgrade = true;
        logic.frame = 6;
        logic.process_normal_updates_with_upgrade_services(None, Some(&mut services));
        let replaced = seen.lock().unwrap().clone();
        assert!(!replaced.is_empty());
        assert!(replaced.iter().all(|id| !placed.contains(id)));
        for id in &placed {
            assert!(logic.dead_objects.contains(id), "old mine {id} queued for destroy");
        }
        for id in &replaced {
            let mine = logic.find_object_by_id(*id).unwrap();
            assert_eq!(mine.read().unwrap().get_template().get_name().as_str(), "SchedEmpMine");
        }
    }


    /// hq-2qedt.72: the REAL GenerateMinefieldBehaviorModule behind the actual
    /// ModuleUpdateProxy reaches the scheduler-lent services on a live tick:
    /// the owner's Upgrade_ChinaEMPMines destroys old mines and places upgraded
    /// ones in the same update (GenerateMinefieldBehavior.cpp:437-475).
    #[test]
    fn module_update_proxy_dispatches_real_minefield_to_owned_services() {
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
            *stores.upgrade_center().write().unwrap() = crate::upgrade::center::UpgradeCenter::new();
            crate::system::engine_stores::with_active_stores(&stores, proxy_body)
        });
    }

    fn proxy_body() {
        use crate::object::behavior::generate_minefield_behavior::GenerateMinefieldBehaviorModule;
        let mut definitions = ThingFactory::new();
        assert_eq!(definitions.load_ini_text("Object ProxyMine\n KindOf = MINE\n Geometry = CYLINDER\n GeometryMajorRadius = 3.0\n GeometryMinorRadius = 3.0\n GeometryHeight = 1.0\nEnd\nObject ProxyEmpMine\n KindOf = MINE\n Geometry = CYLINDER\n GeometryMajorRadius = 3.0\n GeometryMinorRadius = 3.0\n GeometryHeight = 1.0\nEnd\nObject ProxyMinefieldSource\n Geometry = CYLINDER\n GeometryMajorRadius = 20.0\n GeometryMinorRadius = 20.0\n GeometryHeight = 1.0\nEnd\n"), 3);
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![24; 64 * 64];
        map.boundaries = vec![crate::common::ICoord2D::new(64, 64)];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_geometry(map);
        let mut factory = ObjectFactory::new();
        let modules = ModuleFactory::new();
        let bodies = Arc::new(
            BodyDefinitionCatalog::new(
                Some(&game_engine::common::global_data::GlobalData::default()),
                HashMap::new(),
                HashMap::new(),
            )
            .unwrap(),
        );
        let mut collision = CollisionSystem::new();
        let mut random = RandomState::default();
        let mut logic = GameLogic::new();
        let source = crate::helpers::adapt_owned_thing_template(
            definitions.find_template("ProxyMinefieldSource", false).unwrap(),
        );
        let mut lookup = |_name: &str| Ok(None);
        let owner_id = factory
            .prepare_object_owned(
                &mut logic,
                source,
                Coord3D::ZERO,
                None,
                ObjectCreationFlags::NO_AI,
                &mut lookup,
                bodies.clone(),
                &modules,
            )
            .unwrap();
        let mut services = OwnedMinefieldPlacementServices {
            factory: &mut factory,
            definitions: &definitions,
            modules: &modules,
            bodies: bodies.clone(),
            terrain: &terrain,
            collision: &mut collision,
            random: &mut random,
        };
        let owner = logic.find_object_by_id(owner_id).unwrap();
        services
            .position(owner_id, &owner, Coord3D::new(128.0, 128.0, 3.0), 0.0)
            .unwrap();

        let data = game_engine::common::thing::module::ModuleDataHandle::new(GenerateMinefieldBehaviorModuleData {
            mine_name: "ProxyMine".into(),
            mine_name_upgraded: Some("ProxyEmpMine".into()),
            distance_around_object: 8.0,
            mines_per_square_foot: 0.01,
            random_jitter: 0.0,
            skip_if_this_much_under_structure: 0.33,
            border_only: true,
            always_circular: true,
            upgradable: true,
            ..GenerateMinefieldBehaviorModuleData::default()
        });
        // Initial field through the same scheduler window (C++ placeMines).
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let initial = Arc::new(RwLock::new(ScheduledMinefieldModule {
            behavior: GenerateMinefieldBehavior::new_from_handle(owner_id, data.clone()),
            upgrade: false,
            mid_update_ids: Arc::clone(&seen),
        }));
        let handle: UpdateModulePtr = initial.clone();
        logic.register_normal_update_module(owner_id, handle.clone());
        logic.frame = 4;
        logic.process_normal_updates_with_upgrade_services(None, Some(&mut services));
        logic.unregister_update_module(owner_id, handle);
        let placed = seen.lock().unwrap().clone();
        let behavior = std::mem::replace(
            &mut initial.write().unwrap().behavior,
            GenerateMinefieldBehavior::new_from_config(0, GenerateMinefieldBehaviorModuleData::default()),
        );
        assert!(!placed.is_empty(), "initial mines placed");

        let module_name = crate::common::AsciiString::from("GenerateMinefieldBehavior");
        let real = GenerateMinefieldBehaviorModule::new(behavior, &module_name, data.clone());
        let (_host, proxy) = crate::object::Object::installed_update_proxy_for_test(
            owner_id,
            "GenerateMinefieldBehavior",
            Box::new(real),
            data,
        );
        logic.register_normal_update_module(owner_id, proxy);

        // Tick without the upgrade: nothing changes.
        logic.frame = 5;
        logic.process_normal_updates_with_upgrade_services(None, Some(&mut services));
        for id in &placed {
            assert!(logic.find_object_by_id(*id).is_some());
            assert!(!logic.dead_objects.contains(id));
        }

        let upgrade = crate::upgrade::center::with_upgrade_center_mut(|center| {
            center.new_upgrade("Upgrade_ChinaEMPMines".into())
        });
        owner.write().unwrap().give_upgrade(&upgrade);
        let counter = logic.get_object_id_counter();
        logic.frame = 6;
        logic.process_normal_updates_with_upgrade_services(None, Some(&mut services));
        for id in &placed {
            assert!(logic.dead_objects.contains(id), "old mine {id} destroyed same update");
        }
        let fresh: Vec<ObjectID> = (counter..logic.get_object_id_counter()).collect();
        assert!(!fresh.is_empty(), "upgraded mines placed through proxy");
        for id in &fresh {
            let mine = logic.find_object_by_id(*id).expect("upgraded mine admitted same update");
            let mine = mine.read().unwrap();
            assert_eq!(mine.get_template().get_name().as_str(), "ProxyEmpMine");
            assert_eq!(mine.get_producer_id(), owner_id);
        }
    }
}
