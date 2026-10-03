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
use crate::helpers::{TheGameLogic, TheThingFactory, get_game_logic_random_value};
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
    TransportAIUpdate, TransportAIUpdateModuleData, WanderAIUpdate, WanderAIUpdateModuleData,
    WorkerAIUpdateModuleData,
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
    ModuleFactory, get_module_factory, init_module_factory,
};
use log::warn;

/// Unified object wrapper that can hold any object type
pub enum GameObjectInstance {
    /// Owned by the factory registry (borrow via get_object / get_object_mut).
    Unit(Unit),
    /// Owned by the factory registry (borrow via get_object_mut).
    Structure(Structure),
    /// Owned by the factory registry (borrow via get_object_mut).
    SimpleObject(SimpleObject),
    /// Base entry: identity only; resolve Object via registry.
    BaseObject(ObjectID),
    /// Projectile classification survives canonical lookup retirement.
    Projectile(ObjectID),
}

impl std::fmt::Debug for GameObjectInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameObjectInstance::Unit(_) => f.write_str("GameObjectInstance::Unit(..)"),
            GameObjectInstance::Structure(_) => f.write_str("GameObjectInstance::Structure(..)"),
            GameObjectInstance::SimpleObject(_) => {
                f.write_str("GameObjectInstance::SimpleObject(..)")
            }
            GameObjectInstance::BaseObject(id) => write!(f, "GameObjectInstance::BaseObject({id})"),
            GameObjectInstance::Projectile(id) => write!(f, "GameObjectInstance::Projectile({id})"),
        }
    }
}

impl GameObjectInstance {
    /// Get the base object reference
    pub fn get_base_object(&self) -> Option<Arc<RwLock<Object>>> {
        match self {
            GameObjectInstance::Unit(unit) => unit.base_object(),
            GameObjectInstance::Structure(structure) => structure.base_object(),
            GameObjectInstance::SimpleObject(simple_object) => simple_object.base_object(),
            GameObjectInstance::BaseObject(id) | GameObjectInstance::Projectile(id) => {
                crate::object::registry::OBJECT_REGISTRY
                    .get_object(*id)
                    .or_else(|| crate::helpers::TheGameLogic::find_object_by_id(*id))
            }
        }
    }

    /// Get object ID
    pub fn get_id(&self) -> ObjectID {
        match self {
            GameObjectInstance::BaseObject(id) | GameObjectInstance::Projectile(id) => *id,
            _ => self
                .get_base_object()
                .and_then(|arc| arc.read().ok().map(|guard| guard.get_id()))
                .unwrap_or(INVALID_ID),
        }
    }

    /// Update the object for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            GameObjectInstance::Unit(unit) => {
                unit.update(delta_time)?;
            }
            GameObjectInstance::Structure(structure) => {
                structure.update(delta_time)?;
            }
            GameObjectInstance::SimpleObject(simple_object) => {
                simple_object.update(delta_time)?;
            }
            GameObjectInstance::BaseObject(_) | GameObjectInstance::Projectile(_) => {
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
        match self {
            Self::Projectile(_) => true,
            Self::BaseObject(_) => false,
            // Preserve unusual authored combinations: a vehicle/structure
            // classification can also carry KINDOF_PROJECTILE.
            _ => self
                .get_base_object()
                .and_then(|object| {
                    object
                        .read()
                        .ok()
                        .map(|object| object.is_kind_of(KindOf::Projectile))
                })
                .unwrap_or(false),
        }
    }

    fn object_type(&self) -> ObjectType {
        match self {
            Self::Unit(_) => ObjectType::Unit,
            Self::Structure(_) => ObjectType::Structure,
            Self::SimpleObject(_) => ObjectType::SimpleObject,
            Self::BaseObject(_) => ObjectType::BaseObject,
            Self::Projectile(_) => ObjectType::Projectile,
        }
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
    pool_stats: HashMap<ObjectType, PoolStats>,
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
        self.create_object_with_status(
            template_name,
            position,
            team,
            flags,
            ObjectStatusMaskType::NONE,
        )
    }

    /// C++ `friend_createObject(tmplate, statusBits, team)` — caller bits are
    /// on the object before `CreateModuleInterface::onCreate`.
    pub fn create_object_with_status(
        &mut self,
        template_name: &str,
        position: Coord3D,
        team: Option<Arc<RwLock<Team>>>,
        flags: ObjectCreationFlags,
        extra_status: ObjectStatusMaskType,
    ) -> Result<ObjectID, Box<dyn std::error::Error + Send + Sync>> {
        // Get template
        let template = self.get_or_load_template(template_name)?;

        // Determine object type from template
        let object_type = self.determine_object_type(&template);

        // Allocate object ID
        let object_id = self.allocate_object_id();

        // Create base object first
        let mut status_mask = template.get_initial_object_status();
        status_mask |= extra_status;
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

        // new_with_id has already installed modules and run onObjectCreated,
        // before GameLogic admission, as in C++ Object.cpp:388-476.
        // Run onCreate hooks on those same instances.
        {
            let mut obj_guard = base_object
                .write()
                .map_err(|e| format!("object write lock poisoned: {}", e))?;
            obj_guard.init_object()?;
        }

        // Create appropriate specialized object
        let game_object = match object_type {
            ObjectType::Unit => {
                let mut unit = Unit::new(base_object.clone(), &template)?;

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

                    let transport_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "TransportAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<TransportAIUpdateModuleData>()
                        })
                        .cloned();
                    let has_transport_ai = transport_ai_module_data.is_some();

                    let wander_ai_module_data = template
                        .get_behavior_module_info()
                        .iter()
                        .find(|entry| entry.name.as_str() == "WanderAIUpdate")
                        .and_then(|entry| {
                            entry
                                .data
                                .as_ref()
                                .downcast_ref::<WanderAIUpdateModuleData>()
                        })
                        .cloned();
                    let has_wander_ai = wander_ai_module_data.is_some();

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

                    let ai_update_module_data = ai_update_module_data.or_else(|| {
                        transport_ai_module_data
                            .as_ref()
                            .map(|data| data.base.clone())
                    });

                    let ai_update_module_data = ai_update_module_data
                        .or_else(|| wander_ai_module_data.as_ref().map(|data| data.base.clone()));

                    let ai_update = Arc::new(Mutex::new(UnitAIUpdate::new(
                        object_id,
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

                GameObjectInstance::Unit(unit)
            }

            ObjectType::Structure => {
                let structure = Structure::new(base_object.clone(), &template)?;
                GameObjectInstance::Structure(structure)
            }

            ObjectType::SimpleObject => {
                let simple_object = SimpleObject::new(base_object.clone(), &template)?;
                GameObjectInstance::SimpleObject(simple_object)
            }

            ObjectType::BaseObject => GameObjectInstance::BaseObject(object_id),

            ObjectType::Projectile => GameObjectInstance::Projectile(object_id),
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

    /// Request canonical destruction; finalization remains at GameLogic's
    /// processDestroyList boundary. No object guard spans either callback phase.
    pub fn destroy_object(
        &mut self,
        game_logic: &mut crate::system::game_logic::GameLogic,
        object_id: ObjectID,
    ) {
        if self.object_registry.contains_key(&object_id) {
            game_logic.destroy_object(object_id);
            self.track_destroyed_object(object_id);
        }
    }

    /// The application adapter releases its factory guard before driving
    /// callbacks, then records only this bookkeeping after the request.
    pub(crate) fn track_destroyed_object(&mut self, object_id: ObjectID) {
        if self.object_registry.contains_key(&object_id)
            && !self.destruction_queue.contains(&object_id)
        {
            self.destruction_queue.push(object_id);
        }
    }

    /// Update all objects for one frame
    pub fn update_all_objects(
        &mut self,
        game_logic: &mut crate::system::game_logic::GameLogic,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Collect object IDs to avoid borrowing issues
        let object_ids: Vec<ObjectID> = self.object_registry.keys().cloned().collect();

        for object_id in object_ids {
            if let Some(game_object) = self.object_registry.get_mut(&object_id) {
                if let Err(e) = game_object.update(delta_time) {
                    // If update fails (e.g., projectile should be destroyed), mark for destruction
                    if e.to_string().contains("should be destroyed") {
                        self.destroy_object(game_logic, object_id);
                    } else {
                        eprintln!("Error updating object {}: {}", object_id, e);
                    }
                }
            }
        }

        // Process destruction queue
        self.process_destruction_queue(game_logic);

        Ok(())
    }

    /// Retire factory wrappers only after the driving owner retires admission.
    /// The factory never unregisters lookup or finalizes gameplay objects.
    fn process_destruction_queue(&mut self, game_logic: &crate::system::game_logic::GameLogic) {
        let destroyed_ids = std::mem::take(&mut self.destruction_queue);
        for object_id in destroyed_ids {
            if game_logic.contains_object_id(object_id) {
                self.destruction_queue.push(object_id);
                continue;
            }
            if let Some(game_object) = self.object_registry.remove(&object_id) {
                self.update_pool_stats_destroyed(&game_object.object_type());
                self.total_objects_destroyed += 1;
            }
        }
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
        let mut stats = ObjectFactoryStats {
            total_objects: self.object_registry.len() as u32,
            total_created: self.total_objects_created,
            total_destroyed: self.total_objects_destroyed,
            units: 0,
            structures: 0,
            projectiles: 0,
            simple_objects: 0,
            pool_stats: self
                .pool_stats
                .iter()
                .map(|(kind, pool)| (kind.to_string(), pool.clone()))
                .collect(),
        };
        for object in self.object_registry.values() {
            stats.units += u32::from(object.is_unit());
            stats.structures += u32::from(object.is_structure());
            stats.projectiles += u32::from(object.is_projectile());
            stats.simple_objects += u32::from(object.is_simple_object());
        }
        stats
    }

    /// Reset this factory's objects in canonical C++ linked-list order.
    pub fn clear_all_objects(
        &mut self,
        game_logic: &mut crate::system::game_logic::GameLogic,
    ) -> Result<(), crate::system::game_logic::GameLogicError> {
        let admitted: Vec<_> = game_logic
            .get_all_object_ids()
            .iter()
            .copied()
            .filter(|id| self.object_registry.contains_key(id))
            .collect();
        for id in admitted {
            self.destroy_object(game_logic, id);
        }
        game_logic.cleanup_dead_objects()?;
        // Include wrappers whose canonical lifetime already ended elsewhere.
        let retired: Vec<_> = self.object_registry.keys().copied().collect();
        for id in retired {
            self.track_destroyed_object(id);
        }
        self.process_destruction_queue(game_logic);
        // A factory-only reset cannot restart an ID namespace still in use.
        if game_logic.get_object_count() == 0 {
            self.next_object_id = 1;
        }
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
        let mut drawable = Drawable::new(
            drawable_id,
            object_id,
            model_name.to_string(),
            drawable_type,
        );
        // This legacy factory is still reached through a process singleton.
        // Capture its currently executing world once at creation; the retained
        // Drawable never resolves ambient state when weapon recoil fires.
        let visual_owner =
            crate::helpers::ClientVisualHandle::new(crate::system::engine_stores::active())
                .downgrade();
        drawable.bind_visual_owner(visual_owner);
        let drawable = Arc::new(RwLock::new(drawable));

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

                if draw_module_below_min_lod(module_data.as_ref()) {
                    continue;
                }

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
                        module_name, object_id
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
                            warn!(
                                "ModuleFactory still not initialised after retry while creating draw modules"
                            );
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

        // Match C++ bindObjectAndDrawable ordering: draw modules are created
        // first, then receive their Object association and bound callback.
        if let Ok(mut guard) = drawable.write() {
            guard.friend_bind_to_object(base_object);
        }

        // Associate drawable with object
        if let Ok(mut obj_guard) = base_object.write() {
            obj_guard.set_drawable(Some(Arc::clone(&drawable)));
        }

        Ok(())
    }

    fn update_pool_stats(&mut self, object_type: &ObjectType) {
        let stats = self.pool_stats.entry(*object_type).or_default();
        stats.allocated += 1;
        stats.in_use += 1;
        stats.peak_usage = stats.peak_usage.max(stats.in_use);
    }

    fn update_pool_stats_destroyed(&mut self, object_type: &ObjectType) {
        if let Some(stats) = self.pool_stats.get_mut(object_type) {
            stats.in_use = stats.in_use.saturating_sub(1);
        }
    }
}

/// Object type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ObjectType {
    BaseObject,
    Unit,
    Structure,
    Projectile,
    SimpleObject,
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BaseObject => "BaseObject",
            Self::Unit => "Unit",
            Self::Structure => "Structure",
            Self::Projectile => "Projectile",
            Self::SimpleObject => "SimpleObject",
        })
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

/// C++ Drawable ctor: skip when Extra Animations is off and MinLODRequired is above static LOD.
fn draw_module_below_min_lod(data: &dyn ModuleData) -> bool {
    if !game_engine::common::global_data::read().use_draw_module_lod {
        return false;
    }
    let required = data.get_minimum_required_game_lod() as i32;
    let current = match game_engine::common::game_lod::get_static_lod()
        .to_ascii_uppercase()
        .as_str()
    {
        "LOW" => 0,
        "HIGH" => 2,
        _ => 1,
    };
    required > current
}

#[cfg(test)]
#[path = "object_factory_tests.rs"]
mod tests;
