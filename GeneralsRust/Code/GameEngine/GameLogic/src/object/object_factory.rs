//! Object Factory - Creates and manages the complete object hierarchy
//!
//! This factory is responsible for creating the appropriate object types
//! (Unit, Structure, Projectile, SimpleObject) based on templates and
//! managing their lifecycle according to the C++ implementation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::common::*;
use crate::common::{DefaultThingTemplate, ThingTemplate};
use crate::error::GameLogicError as GameError;
use crate::helpers::{get_game_logic_random_value, TheGameLogic, TheThingFactory};
use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
use crate::object::simple_object::{SimpleObject, SimpleObjectExt};
use crate::object::structure::{Structure, StructureExt};
use crate::object::unit::{Unit, UnitAIUpdate, UnitExt};
use crate::object::update::{
    AIUpdateModuleData, AssaultTransportAIUpdate, AssaultTransportAIUpdateData,
    AssaultTransportAIUpdateModuleData, ChinookAIUpdate, ChinookAIUpdateData,
    ChinookAIUpdateModuleData, DeliverPayloadAIUpdate, DeliverPayloadAIUpdateModuleData,
    DeployStyleAIUpdate, DeployStyleAIUpdateData, DeployStyleAIUpdateModuleData, DozerAIUpdate,
    DozerAIUpdateData, DozerAIUpdateModuleData, HackInternetAIUpdate, HackInternetAIUpdateData,
    HackInternetAIUpdateModuleData, JetAIUpdate, JetAIUpdateModuleData, RailedTransportAIUpdate,
    RailedTransportAIUpdateData, RailedTransportAIUpdateModuleData, SupplyTruckAIUpdateModuleData,
    TransportAIUpdate, WanderAIUpdate, WorkerAIUpdateModuleData,
};
use crate::object::{self, Object, ObjectID};
use crate::player::PlayerIndex;
#[cfg(feature = "allow_surrender")]
use crate::pow_truck_ai_update::{
    POWTruckAIUpdate, POWTruckAIUpdateData, POWTruckAIUpdateModuleData,
};
use crate::supply_system::{
    SupplyTruckAIUpdate, SupplyTruckAIUpdateData, WorkerAIUpdate, WorkerAIUpdateData,
};
use crate::team::Team;
use crate::weapon::WeaponTemplate;
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, ModuleType, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    get_module_factory, init_module_factory, ModuleFactory,
};
use log::warn;

/// Unified object wrapper that can hold any object type
#[derive(Debug)]
pub enum GameObjectInstance {
    Unit(Arc<RwLock<Unit>>),
    Structure(Arc<RwLock<Structure>>),
    SimpleObject(Arc<RwLock<SimpleObject>>),
    BaseObject(Arc<RwLock<Object>>),
}

impl GameObjectInstance {
    /// Get the base object reference
    pub fn get_base_object(&self) -> Option<Arc<RwLock<Object>>> {
        match self {
            GameObjectInstance::Unit(unit) => Some(
                unit.read()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .base_object(),
            ),
            GameObjectInstance::Structure(structure) => structure
                .read()
                .unwrap_or_else(|poison| poison.into_inner())
                .base_object(),
            GameObjectInstance::SimpleObject(simple_object) => simple_object
                .read()
                .unwrap_or_else(|poison| poison.into_inner())
                .base_object(),
            GameObjectInstance::BaseObject(object) => Some(object.clone()),
        }
    }

    /// Get object ID
    pub fn get_id(&self) -> ObjectID {
        self.get_base_object()
            .and_then(|arc| arc.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(INVALID_ID)
    }

    /// Update the object for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            GameObjectInstance::Unit(unit) => {
                if let Ok(mut unit_guard) = unit.write() {
                    unit_guard.update(delta_time)?;
                }
            }
            GameObjectInstance::Structure(structure) => {
                if let Ok(mut structure_guard) = structure.write() {
                    structure_guard.update(delta_time)?;
                }
            }
            GameObjectInstance::SimpleObject(simple_object) => {
                if let Ok(mut simple_object_guard) = simple_object.write() {
                    simple_object_guard.update(delta_time)?;
                }
            }
            GameObjectInstance::BaseObject(_) => {
                // Base objects don't have additional update logic beyond their modules
            }
        }

        Ok(())
    }

    /// Check if this object is of a specific type
    pub fn is_unit(&self) -> bool {
        matches!(self, GameObjectInstance::Unit(_))
    }

    pub fn is_structure(&self) -> bool {
        matches!(self, GameObjectInstance::Structure(_))
    }

    pub fn is_projectile(&self) -> bool {
        self.get_base_object()
            .and_then(|arc| {
                arc.read()
                    .ok()
                    .map(|object| object.is_kind_of(KindOf::Projectile))
            })
            .unwrap_or(false)
    }

    pub fn is_simple_object(&self) -> bool {
        matches!(self, GameObjectInstance::SimpleObject(_))
    }
}

// Object creation flags
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ObjectCreationFlags: u32 {
        const NONE = 0;
        const FROM_TEMPLATE = 1 << 0;      // Create from thing template
        const FROM_SAVE_DATA = 1 << 1;     // Loading from save game
        const EDITOR_OBJECT = 1 << 2;      // Editor-created object
        const SCRIPTED = 1 << 3;           // Created by script
        const NO_DRAWABLE = 1 << 4;        // Don't create drawable
        const NO_AI = 1 << 5;             // Don't create AI modules
        const TEMPORARY = 1 << 6;          // Temporary object
        const NO_PHYSICS = 1 << 7;         // Don't apply physics
        const NO_COLLISION = 1 << 8;       // Don't check collisions
        const IGNORE_PREREQUISITES = 1 << 9; // Ignore build prerequisites
    }
}

/// Object Factory responsible for creating all game objects
pub struct ObjectFactory {
    /// Next available object ID
    next_object_id: ObjectID,

    /// Registry of all created objects
    object_registry: HashMap<ObjectID, GameObjectInstance>,

    /// Template cache for performance
    template_cache: HashMap<String, Arc<dyn ThingTemplate>>,

    /// Weapon template cache
    #[allow(dead_code)]
    weapon_template_cache: HashMap<String, Arc<WeaponTemplate>>,

    /// Objects to be destroyed at end of frame
    destruction_queue: Vec<ObjectID>,

    /// Creation statistics
    total_objects_created: u32,
    total_objects_destroyed: u32,

    /// Memory pool statistics
    pool_stats: HashMap<String, PoolStats>,
}

/// Memory pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub allocated: u32,
    pub in_use: u32,
    pub peak_usage: u32,
}

impl ObjectFactory {
    /// Create a new ObjectFactory
    pub fn new() -> Self {
        ObjectFactory {
            next_object_id: 1, // Start from 1, as 0 is INVALID_ID
            object_registry: HashMap::new(),
            template_cache: HashMap::new(),
            weapon_template_cache: HashMap::new(),
            destruction_queue: Vec::new(),
            total_objects_created: 0,
            total_objects_destroyed: 0,
            pool_stats: HashMap::new(),
        }
    }

    /// Create object from template
    pub fn create_object(
        &mut self,
        template_name: &str,
        position: Coord3D,
        team: Option<Arc<RwLock<Team>>>,
        flags: ObjectCreationFlags,
    ) -> Result<ObjectID, Box<dyn std::error::Error + Send + Sync>> {
        // Get template
        let template = self.get_or_load_template(template_name)?;

        // Determine object type from template
        let object_type = self.determine_object_type(&template);

        // Allocate object ID
        let object_id = self.allocate_object_id();

        // Create base object first
        let status_mask = template.get_initial_object_status();
        let base_object =
            Object::new_with_id(template.clone(), object_id, status_mask, team.clone())?;

        // Set object ID and position
        {
            let mut obj_guard = base_object
                .write()
                .map_err(|e| format!("object write lock poisoned: {}", e))?;
            obj_guard.set_position(&position)?;
        }

        // Register the base object with the global GameLogic singleton
        TheGameLogic::register_object(base_object.clone())
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?;

        // Instantiate behavior/update/etc modules from template descriptors.
        Object::init_modules_for(&base_object, template.as_ref())?;

        // Run object initialization hooks after modules are attached.
        {
            let mut obj_guard = base_object
                .write()
                .map_err(|e| format!("object write lock poisoned: {}", e))?;
            obj_guard.init_object()?;
        }

        // Create appropriate specialized object
        let game_object = match object_type {
            ObjectType::Unit => {
                let unit = Unit::new(base_object.clone(), &template)?;
                let unit_arc = Arc::new(RwLock::new(unit));

                if !flags.contains(ObjectCreationFlags::NO_AI) {
                    let needs_supply_ai = template.is_kind_of(KindOf::Harvester);
                    #[cfg(feature = "allow_surrender")]
                    let needs_pow_truck_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "POWTruckBehavior");

                    let ai_update_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "AIUpdateInterface")
                        .and_then(|entry| entry.data.as_ref().downcast_ref::<AIUpdateModuleData>())
                        .map(|data| data.clone());

                    let railed_transport_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "RailedTransportAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<RailedTransportAIUpdateModuleData>()
                        })
                        .cloned();

                    let hack_internet_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "HackInternetAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<HackInternetAIUpdateModuleData>()
                        })
                        .cloned();

                    let assault_transport_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "AssaultTransportAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<AssaultTransportAIUpdateModuleData>()
                        })
                        .cloned();

                    let deliver_payload_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "DeliverPayloadAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
                        })
                        .cloned();

                    let deploy_style_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "DeployStyleAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<DeployStyleAIUpdateModuleData>()
                        })
                        .cloned();

                    let has_transport_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "TransportAIUpdate");

                    let has_wander_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "WanderAIUpdate");

                    let dozer_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "DozerAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<DozerAIUpdateModuleData>()
                        })
                        .cloned();

                    let chinook_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<ChinookAIUpdateModuleData>()
                        })
                        .cloned();

                    let jet_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "JetAIUpdate")
                        .and_then(|entry| {
                            entry.data.as_ref().downcast_ref::<JetAIUpdateModuleData>()
                        })
                        .cloned();

                    let supply_truck_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "SupplyTruckAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<SupplyTruckAIUpdateModuleData>()
                        })
                        .cloned();

                    let worker_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "WorkerAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<WorkerAIUpdateModuleData>()
                        })
                        .cloned();

                    #[cfg(feature = "allow_surrender")]
                    let pow_truck_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "POWTruckAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<POWTruckAIUpdateModuleData>()
                        })
                        .map(|data| data.base.clone());

                    let needs_worker_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "WorkerAIUpdate");

                    let needs_dozer_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "DozerAIUpdate");

                    let needs_chinook_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "ChinookAIUpdate");

                    let needs_jet_ai = template
                        .get_behavior_module_info()
                        .iter()
                        .any(|entry| entry.name.as_str() == "JetAIUpdate");

                    let supply_ai = if needs_supply_ai {
                        let player_index = base_object
                            .read()
                            .ok()
                            .and_then(|obj| obj.get_controlling_player_id())
                            .unwrap_or(0) as PlayerIndex;
                        let data = supply_truck_ai_module_data.clone().map_or_else(
                            SupplyTruckAIUpdateData::default,
                            |data| SupplyTruckAIUpdateData {
                                max_boxes: data.max_boxes_data,
                                warehouse_scan_distance: data.warehouse_scan_distance,
                                warehouse_delay: data.warehouse_delay,
                                center_delay: data.center_delay,
                                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
                            },
                        );
                        Some(SupplyTruckAIUpdate::new(
                            data,
                            object_id,
                            player_index as crate::supply_system::PlayerIndex,
                        ))
                    } else {
                        None
                    };

                    let worker_ai = if needs_worker_ai {
                        let player_index = base_object
                            .read()
                            .ok()
                            .and_then(|obj| obj.get_controlling_player_id())
                            .unwrap_or(0) as PlayerIndex;
                        let data = worker_ai_module_data.clone().map_or_else(
                            WorkerAIUpdateData::default,
                            |data| WorkerAIUpdateData {
                                max_boxes: data.max_boxes_data,
                                warehouse_scan_distance: data.warehouse_scan_distance,
                                warehouse_delay: data.warehouse_delay,
                                center_delay: data.center_delay,
                                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
                                repair_health_percent_per_second: data
                                    .repair_health_percent_per_second,
                                bored_time: data.bored_time,
                                bored_range: data.bored_range,
                                upgraded_supply_boost: data.upgraded_supply_boost.max(0) as u32,
                            },
                        );
                        Some(WorkerAIUpdate::new(
                            data,
                            object_id,
                            player_index as crate::supply_system::PlayerIndex,
                        ))
                    } else {
                        None
                    };

                    let dozer_ai = if needs_dozer_ai {
                        let data = dozer_ai_module_data.clone().map_or_else(
                            DozerAIUpdateData::default,
                            |data| DozerAIUpdateData {
                                repair_health_percent_per_second: data
                                    .repair_health_percent_per_second,
                                bored_time: data.bored_time,
                                bored_range: data.bored_range,
                            },
                        );
                        Some(DozerAIUpdate::new(data, object_id))
                    } else {
                        None
                    };

                    let mut chinook_ai = if needs_chinook_ai {
                        let player_index = base_object
                            .read()
                            .ok()
                            .and_then(|obj| obj.get_controlling_player_id())
                            .unwrap_or(0) as PlayerIndex;
                        let data = chinook_ai_module_data
                            .clone()
                            .map_or_else(ChinookAIUpdateData::default, |data| {
                                ChinookAIUpdateData::from_module(&data)
                            });
                        Some(ChinookAIUpdate::new(data, object_id, player_index))
                    } else {
                        None
                    };
                    if let Some(ref mut chinook_ai) = chinook_ai {
                        if let Ok(obj_guard) = base_object.read() {
                            chinook_ai.record_original_position(*obj_guard.get_position());
                        }
                    }

                    let jet_ai = if needs_jet_ai {
                        jet_ai_module_data
                            .as_ref()
                            .map(|data| JetAIUpdate::new(data.clone(), object_id))
                    } else {
                        None
                    };

                    #[cfg(feature = "allow_surrender")]
                    let pow_truck_ai = if needs_pow_truck_ai {
                        let data =
                            pow_truck_ai_module_data.unwrap_or_else(POWTruckAIUpdateData::default);
                        Some(POWTruckAIUpdate::new(data, object_id))
                    } else {
                        None
                    };

                    let railed_transport_ai =
                        railed_transport_ai_module_data.as_ref().map(|data| {
                            let data = RailedTransportAIUpdateData {
                                path_prefix_name: data.path_prefix_name.clone(),
                            };
                            RailedTransportAIUpdate::new(data, object_id)
                        });

                    let hack_internet_ai = hack_internet_ai_module_data.as_ref().map(|data| {
                        let data = HackInternetAIUpdateData {
                            unpack_time: data.unpack_time,
                            pack_time: data.pack_time,
                            cash_update_delay: data.cash_update_delay,
                            cash_update_delay_fast: data.cash_update_delay_fast,
                            regular_cash_amount: data.regular_cash_amount,
                            veteran_cash_amount: data.veteran_cash_amount,
                            elite_cash_amount: data.elite_cash_amount,
                            heroic_cash_amount: data.heroic_cash_amount,
                            xp_per_cash_update: data.xp_per_cash_update,
                            pack_unpack_variation_factor: data.pack_unpack_variation_factor,
                        };
                        HackInternetAIUpdate::new(data, object_id)
                    });

                    let assault_transport_ai =
                        assault_transport_ai_module_data.as_ref().map(|data| {
                            let data = AssaultTransportAIUpdateData {
                                members_get_healed_at_life_ratio: data
                                    .members_get_healed_at_life_ratio,
                                clear_range_required_to_continue_attack_move: data
                                    .clear_range_required_to_continue_attack_move,
                            };
                            AssaultTransportAIUpdate::new(data, object_id)
                        });

                    let deliver_payload_ai = deliver_payload_ai_module_data
                        .as_ref()
                        .map(|data| DeliverPayloadAIUpdate::new(data.clone(), object_id));

                    let deploy_style_ai = deploy_style_ai_module_data.as_ref().map(|data| {
                        let data = DeployStyleAIUpdateData {
                            unpack_time: data.unpack_time,
                            pack_time: data.pack_time,
                            reset_turret_before_packing: data.reset_turret_before_packing,
                            turrets_function_only_when_deployed: data
                                .turrets_function_only_when_deployed,
                            turrets_must_center_before_packing: data
                                .turrets_must_center_before_packing,
                            manual_deploy_animations: data.manual_deploy_animations,
                        };
                        DeployStyleAIUpdate::new(data, object_id)
                    });

                    let transport_ai = has_transport_ai.then(|| TransportAIUpdate::new(object_id));
                    let wander_ai = has_wander_ai.then(|| WanderAIUpdate::new(object_id));

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        railed_transport_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        hack_internet_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        assault_transport_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        deliver_payload_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        deploy_style_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        chinook_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data
                        .or_else(|| jet_ai_module_data.as_ref().map(|data| data.base.clone()));

                    let ai_update_module_data = ai_update_module_data
                        .or_else(|| dozer_ai_module_data.as_ref().map(|data| data.base.clone()));

                    let ai_update_module_data = ai_update_module_data
                        .or_else(|| worker_ai_module_data.as_ref().map(|data| data.base.clone()));

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        supply_truck_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update = Arc::new(Mutex::new(UnitAIUpdate::new(
                        Arc::downgrade(&unit_arc),
                        supply_ai,
                        chinook_ai,
                        jet_ai,
                        worker_ai,
                        dozer_ai,
                        #[cfg(feature = "allow_surrender")]
                        pow_truck_ai,
                        railed_transport_ai,
                        hack_internet_ai,
                        assault_transport_ai,
                        deliver_payload_ai,
                        transport_ai,
                        deploy_style_ai,
                        wander_ai,
                    )));

                    if let Some(data) = ai_update_module_data {
                        if let Ok(mut ai_guard) = ai_update.lock() {
                            ai_guard.apply_ai_update_module_data(&data);
                        }
                    }
                    if let Ok(mut obj_guard) = base_object.write() {
                        obj_guard.set_ai_update_interface(Some(ai_update.clone()));
                        obj_guard.attach_ai_update_to_module(ai_update);
                    }
                }

                GameObjectInstance::Unit(unit_arc)
            }

            ObjectType::Structure => {
                let structure = Structure::new(base_object.clone(), &template)?;
                GameObjectInstance::Structure(Arc::new(RwLock::new(structure)))
            }

            ObjectType::SimpleObject => {
                let simple_object = SimpleObject::new(base_object.clone(), &template)?;
                GameObjectInstance::SimpleObject(Arc::new(RwLock::new(simple_object)))
            }

            ObjectType::BaseObject => GameObjectInstance::BaseObject(base_object.clone()),

            ObjectType::Projectile => GameObjectInstance::BaseObject(base_object.clone()),
        };

        // Create drawable if needed
        if !flags.contains(ObjectCreationFlags::NO_DRAWABLE) {
            let base_object_for_drawable = Arc::clone(&base_object);
            self.create_drawable_for_object(object_id, &template, &base_object_for_drawable)?;
        }

        // Collision registration is handled by GameLogic::register_object on base object creation.

        // Register the object
        self.object_registry.insert(object_id, game_object);
        self.total_objects_created += 1;

        // Update pool statistics
        self.update_pool_stats(&object_type);

        Ok(object_id)
    }

    /// Get object by ID
    pub fn get_object(&self, object_id: ObjectID) -> Option<&GameObjectInstance> {
        self.object_registry.get(&object_id)
    }

    /// Get mutable object by ID
    pub fn get_object_mut(&mut self, object_id: ObjectID) -> Option<&mut GameObjectInstance> {
        self.object_registry.get_mut(&object_id)
    }

    /// Destroy object by ID
    pub fn destroy_object(
        &mut self,
        object_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(game_object) = self.object_registry.get(&object_id) {
            // Call destroy callbacks on the base object
            let Some(base_object) = game_object.get_base_object() else {
                return Ok(());
            };
            if let Ok(mut obj_guard) = base_object.write() {
                obj_guard.on_destroy();
                let _ = TheGameLogic::destroy_object(&obj_guard);
            }

            // Add to destruction queue for cleanup at end of frame
            self.destruction_queue.push(object_id);
        }

        Ok(())
    }

    /// Update all objects for one frame
    pub fn update_all_objects(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Collect object IDs to avoid borrowing issues
        let object_ids: Vec<ObjectID> = self.object_registry.keys().cloned().collect();

        for object_id in object_ids {
            if let Some(game_object) = self.object_registry.get_mut(&object_id) {
                if let Err(e) = game_object.update(delta_time) {
                    // If update fails (e.g., projectile should be destroyed), mark for destruction
                    if e.to_string().contains("should be destroyed") {
                        self.destruction_queue.push(object_id);
                    } else {
                        eprintln!("Error updating object {}: {}", object_id, e);
                    }
                }
            }
        }

        // Process destruction queue
        self.process_destruction_queue()?;

        Ok(())
    }

    /// Process objects marked for destruction
    fn process_destruction_queue(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let destroyed_ids: Vec<ObjectID> = self.destruction_queue.drain(..).collect();
        for object_id in destroyed_ids {
            if let Some(game_object) = self.object_registry.remove(&object_id) {
                TheGameLogic::remove_object(object_id);
                // Update statistics based on object type
                match game_object {
                    GameObjectInstance::Unit(_) => {
                        self.update_pool_stats_destroyed(&ObjectType::Unit);
                    }
                    GameObjectInstance::Structure(_) => {
                        self.update_pool_stats_destroyed(&ObjectType::Structure);
                    }
                    GameObjectInstance::SimpleObject(_) => {
                        self.update_pool_stats_destroyed(&ObjectType::SimpleObject);
                    }
                    GameObjectInstance::BaseObject(object) => {
                        let object_type = object
                            .read()
                            .map(|object| {
                                if object.is_kind_of(KindOf::Projectile) {
                                    ObjectType::Projectile
                                } else {
                                    ObjectType::BaseObject
                                }
                            })
                            .unwrap_or(ObjectType::BaseObject);
                        self.update_pool_stats_destroyed(&object_type);
                    }
                }

                self.total_objects_destroyed += 1;
            }
        }

        Ok(())
    }

    /// Get all objects of a specific type
    pub fn get_objects_by_type<F>(&self, type_check: F) -> Vec<ObjectID>
    where
        F: Fn(&GameObjectInstance) -> bool,
    {
        self.object_registry
            .iter()
            .filter(|(_, obj)| type_check(obj))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all units
    pub fn get_all_units(&self) -> Vec<ObjectID> {
        self.get_objects_by_type(|obj| obj.is_unit())
    }

    /// Get all structures
    pub fn get_all_structures(&self) -> Vec<ObjectID> {
        self.get_objects_by_type(|obj| obj.is_structure())
    }

    /// Get all projectiles
    pub fn get_all_projectiles(&self) -> Vec<ObjectID> {
        self.get_objects_by_type(|obj| obj.is_projectile())
    }

    /// Get statistics
    pub fn get_statistics(&self) -> ObjectFactoryStats {
        ObjectFactoryStats {
            total_objects: self.object_registry.len() as u32,
            total_created: self.total_objects_created,
            total_destroyed: self.total_objects_destroyed,
            units: self.get_all_units().len() as u32,
            structures: self.get_all_structures().len() as u32,
            projectiles: self.get_all_projectiles().len() as u32,
            simple_objects: self.get_objects_by_type(|obj| obj.is_simple_object()).len() as u32,
            pool_stats: self.pool_stats.clone(),
        }
    }

    /// Clear all objects (for map changes, etc.)
    pub fn clear_all_objects(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Add all objects to destruction queue
        let all_ids: Vec<ObjectID> = self.object_registry.keys().cloned().collect();
        for id in all_ids {
            self.destruction_queue.push(id);
        }

        // Process destruction queue
        self.process_destruction_queue()?;

        // Reset ID counter
        self.next_object_id = 1;

        Ok(())
    }

    // Private helper methods

    fn allocate_object_id(&mut self) -> ObjectID {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    fn get_or_load_template(
        &mut self,
        template_name: &str,
    ) -> Result<Arc<dyn ThingTemplate>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(template) = self.template_cache.get(template_name) {
            Ok(template.clone())
        } else {
            let template = TheThingFactory::find_template(template_name).ok_or_else(|| {
                Box::<dyn std::error::Error + Send + Sync>::from(GameError::Configuration(format!(
                    "Template not found: {}",
                    template_name
                )))
            })?;

            self.template_cache
                .insert(template_name.to_string(), template.clone());
            Ok(template)
        }
    }

    fn determine_object_type(&self, template: &dyn ThingTemplate) -> ObjectType {
        if template.is_kind_of(KindOf::Vehicle)
            || template.is_kind_of(KindOf::Infantry)
            || template.is_kind_of(KindOf::Aircraft)
        {
            ObjectType::Unit
        } else if template.is_kind_of(KindOf::Structure) {
            ObjectType::Structure
        } else if template.is_kind_of(KindOf::Projectile) {
            ObjectType::Projectile
        } else if template.is_kind_of(KindOf::Crate)
            || template.is_kind_of(KindOf::ResourceNode)
            || template.is_kind_of(KindOf::TechBuilding)
        {
            ObjectType::SimpleObject
        } else {
            ObjectType::BaseObject
        }
    }

    fn create_drawable_for_object(
        &mut self,
        object_id: ObjectID,
        template: &dyn ThingTemplate,
        base_object: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create drawable based on template
        let model_name = template.get_model_name();
        let drawable_type = if template.is_kind_of(KindOf::Structure) {
            DrawableType::Static
        } else {
            DrawableType::Animated
        };

        let drawable_id = Drawable::allocate_drawable_id();
        let drawable = Arc::new(RwLock::new(Drawable::new(
            drawable_id,
            object_id,
            model_name.to_string(),
            drawable_type,
        )));
        if let Ok(mut guard) = drawable.write() {
            guard.bind_object_ref(base_object);
        }

        let _ = template.module_descriptors();

        let module_thing: Arc<dyn ModuleThing> =
            object::make_drawable_module_thing_handle(base_object, &drawable);
        let mut drawable_modules: Vec<(
            ModuleInterfaceType,
            AsciiString,
            AsciiString,
            Arc<dyn ModuleData>,
            Box<dyn Module>,
        )> = Vec::new();

        let mut install_drawable_modules = |factory: &ModuleFactory| {
            for entry in template.get_draw_module_info().iter() {
                let module_name = entry.name.clone();
                let module_data = Arc::clone(&entry.data);
                let module_data_for_entry = Arc::clone(&module_data);
                let interface_mask = entry.interface_flags();

                if factory.find_module_interface_mask(&module_name, ModuleType::Draw)
                    == ModuleInterfaceType::NONE
                {
                    warn!(
                        "Descriptor for draw module '{}' missing during drawable init (object {})",
                        module_name, object_id
                    );
                    continue;
                }

                match factory.new_module(
                    module_thing.clone(),
                    &module_name,
                    module_data,
                    ModuleType::Draw,
                ) {
                    Ok(module) => {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                    Err(err) => warn!(
                        "Failed to instantiate draw module '{}' for object {}: {}",
                        module_name, object_id, err
                    ),
                }
            }

            for entry in template.get_client_update_module_info().iter() {
                let module_name = entry.name.clone();
                let module_data = Arc::clone(&entry.data);
                let module_data_for_entry = Arc::clone(&module_data);
                let interface_mask = entry.interface_flags();

                if factory.find_module_interface_mask(&module_name, ModuleType::ClientUpdate)
                    == ModuleInterfaceType::NONE
                {
                    warn!(
                        "Descriptor for client-update module '{}' missing during drawable init (object {})",
                        module_name,
                        object_id
                    );
                    continue;
                }

                match factory.new_module(
                    module_thing.clone(),
                    &module_name,
                    module_data,
                    ModuleType::ClientUpdate,
                ) {
                    Ok(module) => {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                    Err(err) => warn!(
                        "Failed to instantiate client-update module '{}' for object {}: {}",
                        module_name, object_id, err
                    ),
                }
            }
        };

        let mut installed = false;
        match get_module_factory() {
            Ok(factory_guard) => {
                if let Some(factory) = factory_guard.as_ref() {
                    install_drawable_modules(factory);
                    installed = true;
                }
            }
            Err(_) => warn!("Failed to lock ModuleFactory when creating draw modules"),
        }

        if !installed {
            if init_module_factory().is_ok() {
                match get_module_factory() {
                    Ok(factory_guard) => {
                        if let Some(factory) = factory_guard.as_ref() {
                            install_drawable_modules(factory);
                        } else {
                            warn!("ModuleFactory still not initialised after retry while creating draw modules");
                        }
                    }
                    Err(_) => warn!(
                        "Failed to lock ModuleFactory after retry while creating draw modules"
                    ),
                }
            } else {
                warn!("ModuleFactory initialisation failed while creating draw modules");
            }
        }

        if !drawable_modules.is_empty() {
            match drawable.write() {
                Ok(mut guard) => {
                    for (interface_mask, name, tag, module_data, module) in drawable_modules {
                        let _ = guard.add_module(interface_mask, name, tag, module_data, module);
                    }
                }
                Err(_) => warn!("Drawable lock poisoned while installing draw modules"),
            }
        }

        // Associate drawable with object
        if let Ok(mut obj_guard) = base_object.write() {
            obj_guard.set_drawable(Some(Arc::clone(&drawable)));
        }

        Ok(())
    }

    fn update_pool_stats(&mut self, object_type: &ObjectType) {
        let type_name = object_type.to_string();
        let stats = self.pool_stats.entry(type_name).or_default();
        stats.allocated += 1;
        stats.in_use += 1;
        stats.peak_usage = stats.peak_usage.max(stats.in_use);
    }

    fn update_pool_stats_destroyed(&mut self, object_type: &ObjectType) {
        let type_name = object_type.to_string();
        if let Some(stats) = self.pool_stats.get_mut(&type_name) {
            stats.in_use = stats.in_use.saturating_sub(1);
        }
    }
}

/// Object type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectType {
    BaseObject,
    Unit,
    Structure,
    Projectile,
    SimpleObject,
}

impl ToString for ObjectType {
    fn to_string(&self) -> String {
        match self {
            ObjectType::BaseObject => "BaseObject",
            ObjectType::Unit => "Unit",
            ObjectType::Structure => "Structure",
            ObjectType::Projectile => "Projectile",
            ObjectType::SimpleObject => "SimpleObject",
        }
        .to_string()
    }
}

/// Object factory statistics
#[derive(Debug, Clone)]
pub struct ObjectFactoryStats {
    pub total_objects: u32,
    pub total_created: u32,
    pub total_destroyed: u32,
    pub units: u32,
    pub structures: u32,
    pub projectiles: u32,
    pub simple_objects: u32,
    pub pool_stats: HashMap<String, PoolStats>,
}

// Global object factory instance
lazy_static::lazy_static! {
    pub static ref THE_OBJECT_FACTORY: Arc<RwLock<ObjectFactory>> =
        Arc::new(RwLock::new(ObjectFactory::new()));
}

/// Convenience function to get the global object factory
pub fn get_object_factory() -> Arc<RwLock<ObjectFactory>> {
    THE_OBJECT_FACTORY.clone()
}
