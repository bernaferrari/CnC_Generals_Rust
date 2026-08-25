// FILE: mod.rs - Object Creation List System
// Author: Steven Johnson, December 2001 (C++)
// Rust Port: 2025
// Desc: ObjectCreationList system for spawning objects, debris, projectiles, and effects
//
// Ported from GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp
//
// This system handles:
// - Death debris spawning
// - Unit spawning at rally points
// - Building construction placement
// - Projectile creation
// - Crate drops
// - Reinforcement delivery (airstrikes, paradrops)
// - Special power object creation

pub mod advanced_nuggets;
pub mod nuggets;
pub mod store;

pub use advanced_nuggets::*;
pub use nuggets::*;
pub use store::{
    ObjectCreationList, ObjectCreationListId, ObjectCreationListStore,
    get_object_creation_list_store, get_object_creation_list_store_mut,
    init_object_creation_list_store,
};

pub use crate::common::PathfindLayerEnum;
use crate::common::*;
use crate::object::Object;
use std::sync::{Arc, RwLock};

/// Result of creating objects from a nugget
pub type CreationResult = Option<Arc<RwLock<Object>>>;

/// Context for object creation - provides access to game systems
pub struct CreationContext<'a> {
    pub game_logic: &'a dyn GameLogicContext,
    pub thing_factory: &'a dyn ThingFactoryContext,
    pub terrain_logic: &'a dyn TerrainLogicContext,
}

/// Abstraction over game logic systems for testing
pub trait GameLogicContext {
    fn get_frame(&self) -> UnsignedInt;
    fn random_value(&self, lo: Int, hi: Int) -> Int;
    fn random_value_real(&self, lo: Real, hi: Real) -> Real;
}

/// Abstraction over thing factory for testing
pub trait ThingFactoryContext {
    fn find_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>>;
    fn new_object(
        &self,
        template: Arc<dyn ThingTemplate>,
        team: &Team,
    ) -> Result<Arc<RwLock<Object>>, GameError>;
}

/// Abstraction over terrain logic for testing
pub trait TerrainLogicContext {
    fn get_ground_height(&self, x: Real, y: Real) -> Real;
    fn get_layer_height(&self, x: Real, y: Real, layer: PathfindLayerEnum) -> Real;
    fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum;
    fn is_underwater(&self, x: Real, y: Real, water_z: &mut Real, terrain_z: &mut Real) -> Bool;
    fn flatten_terrain(&self, object: &Object);
    fn find_closest_edge_point(&self, pos: &Coord3D) -> Coord3D;
}

/// Live (singleton-backed) game logic context for OCL creation.
pub struct LiveGameLogicContext;

impl GameLogicContext for LiveGameLogicContext {
    fn get_frame(&self) -> UnsignedInt {
        crate::helpers::TheGameLogic::get_frame()
    }

    fn random_value(&self, lo: Int, hi: Int) -> Int {
        crate::helpers::get_game_logic_random_value(lo, hi)
    }

    fn random_value_real(&self, lo: Real, hi: Real) -> Real {
        crate::helpers::get_game_logic_random_value_real(lo, hi)
    }
}

/// Live (singleton-backed) thing factory context for OCL creation.
pub struct LiveThingFactoryContext;

impl ThingFactoryContext for LiveThingFactoryContext {
    fn find_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>> {
        crate::helpers::TheThingFactory::find_template(name)
    }

    fn new_object(
        &self,
        template: Arc<dyn ThingTemplate>,
        team: &Team,
    ) -> Result<Arc<RwLock<Object>>, GameError> {
        let factory = crate::helpers::TheThingFactory::get()
            .map_err(|e| GameError::SystemError(e.to_string()))?;
        factory
            .new_object(template, team)
            .map_err(|e| GameError::SystemError(e.to_string()))
    }
}

/// Live (singleton-backed) terrain logic context for OCL creation.
pub struct LiveTerrainLogicContext;

impl TerrainLogicContext for LiveTerrainLogicContext {
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| terrain.get_ground_height(x, y, None))
            .unwrap_or(0.0)
    }

    fn get_layer_height(&self, x: Real, y: Real, layer: PathfindLayerEnum) -> Real {
        let layer = match layer {
            PathfindLayerEnum::Air => crate::path::PathfindLayerEnum::Top,
            PathfindLayerEnum::Tunnel | PathfindLayerEnum::Water | PathfindLayerEnum::Last => {
                crate::path::PathfindLayerEnum::Ground
            }
            other => crate::path::PathfindLayerEnum::from_u32(other as u32),
        };
        crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| terrain.get_layer_height(x, y, layer, None, true))
            .unwrap_or_else(|| self.get_ground_height(x, y))
    }

    fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(
                |terrain| match terrain.get_highest_layer_for_destination(pos) {
                    crate::path::PathfindLayerEnum::Last => PathfindLayerEnum::Last,
                    other => PathfindLayerEnum::from_u32(other as u32),
                },
            )
            .unwrap_or(PathfindLayerEnum::Ground)
    }

    fn is_underwater(&self, x: Real, y: Real, water_z: &mut Real, terrain_z: &mut Real) -> Bool {
        crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| {
                let mut wz = 0.0;
                let mut tz = 0.0;
                let underwater = terrain.is_underwater(x, y, Some(&mut wz), Some(&mut tz));
                *water_z = wz;
                *terrain_z = tz;
                underwater
            })
            .unwrap_or(false)
    }

    fn flatten_terrain(&self, object: &Object) {
        // C++ TheTerrainLogic->flattenTerrain(obj). Live TerrainLogic takes an Arc.
        let id = object.get_id();
        let Some(arc) = crate::object::registry::OBJECT_REGISTRY.get_object(id) else {
            return;
        };
        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.flatten_terrain(&arc);
        }
    }

    fn find_closest_edge_point(&self, pos: &Coord3D) -> Coord3D {
        crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| terrain.find_closest_edge_point(pos))
            .unwrap_or(*pos)
    }
}

static LIVE_GAME_LOGIC_CONTEXT: LiveGameLogicContext = LiveGameLogicContext;
static LIVE_THING_FACTORY_CONTEXT: LiveThingFactoryContext = LiveThingFactoryContext;
static LIVE_TERRAIN_LOGIC_CONTEXT: LiveTerrainLogicContext = LiveTerrainLogicContext;

/// Convenience helper that returns a live `CreationContext` backed by engine singletons.
pub fn live_creation_context() -> CreationContext<'static> {
    CreationContext {
        game_logic: &LIVE_GAME_LOGIC_CONTEXT,
        thing_factory: &LIVE_THING_FACTORY_CONTEXT,
        terrain_logic: &LIVE_TERRAIN_LOGIC_CONTEXT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        let ocl = ObjectCreationList::new();
        assert_eq!(
            format!("{:?}", ocl),
            "ObjectCreationList { nugget_count: 0 }",
            "default OCL must start with an empty nugget list"
        );

        let store = ObjectCreationListStore::new();
        assert!(
            store.find_object_creation_list("nonexistent_ocl").is_none(),
            "empty store has no named lists"
        );

        let ctx = live_creation_context();
        let _ = ctx.game_logic.get_frame();
    }
}
