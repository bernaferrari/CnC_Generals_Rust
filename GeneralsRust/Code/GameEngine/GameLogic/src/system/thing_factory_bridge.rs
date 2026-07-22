//! Bridge Common ThingFactory creation into GameLogic/GameClient systems.

use crate::common::{Coord3D, ObjectStatusMaskType as GameLogicStatusMask};
use crate::helpers::{TheGameClient, TheThingFactory};
use crate::object::Object as GameLogicObject;
use crate::object_manager::{get_object_manager, ObjectCreationFlags};
use crate::team::get_team_factory;
use crate::upgrade_legacy::upgrade_mask_for_ascii;
use game_engine::common::thing::module as engine_module;
use game_engine::common::thing::thing_factory::{
    set_drawable_creator, set_object_creator, DrawableCreator, DrawableStatus, ObjectCreator,
    ObjectStatusMaskType, Team as EngineTeam, ThingCreationError,
};
use game_engine::common::thing::thing_template::ThingTemplate;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
struct CommonObjectHandle {
    object: Arc<RwLock<GameLogicObject>>,
}

impl engine_module::Object for CommonObjectHandle {
    fn get_object_id(&self) -> game_engine::common::system::build_assistant::ObjectID {
        self.object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(game_engine::common::system::build_assistant::INVALID_ID)
    }

    fn get_behavior_modules(&self) -> Vec<Arc<dyn engine_module::Module>> {
        self.object
            .read()
            .map(|guard| engine_module::Object::get_behavior_modules(&*guard))
            .unwrap_or_default()
    }

    fn init_object(&self) {
        if let Ok(guard) = self.object.read() {
            guard.init_object();
        }
    }

    fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn engine_module::Object>>> {
        let arc: Arc<RwLock<dyn engine_module::Object>> = self.object.clone();
        Some(arc)
    }

    fn remove_upgrade(
        &self,
        upgrade_template: Option<&game_engine::common::ini::ini_upgrade::UpgradeTemplate>,
    ) {
        let Some(template) = upgrade_template else {
            return;
        };
        let upgrade_name = template.name.as_str();
        if upgrade_name.is_empty() {
            return;
        }

        let mask_bits = upgrade_mask_for_ascii(upgrade_name);
        if mask_bits.is_empty() {
            return;
        }

        if let Ok(mut guard) = self.object.write() {
            guard.remove_upgrade_mask(mask_bits);
        }
    }
}

#[derive(Debug)]
struct CommonDrawableHandle {
    _id: u32,
}

impl engine_module::Drawable for CommonDrawableHandle {}

struct GameLogicObjectCreator;

impl GameLogicObjectCreator {
    fn resolve_team(team: Option<Arc<dyn EngineTeam>>) -> Option<Arc<RwLock<crate::team::Team>>> {
        let team = team?;
        let team_id = team.team_id()?;
        get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(team_id))
    }
}

impl ObjectCreator for GameLogicObjectCreator {
    fn create_object(
        &self,
        template: &ThingTemplate,
        status_bits: ObjectStatusMaskType,
        team: Option<Arc<dyn EngineTeam>>,
    ) -> Result<Box<dyn engine_module::Object>, ThingCreationError> {
        let template_name = template.get_name().to_string();
        let team_arc = Self::resolve_team(team);

        let object_id = get_object_manager()
            .write()
            .map_err(|_| {
                ThingCreationError::CreationFailed("ObjectManager lock poisoned".to_string())
            })?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                team_arc,
                ObjectCreationFlags::from_template(),
            )
            .map_err(|e| ThingCreationError::CreationFailed(e.to_string()))?;

        let instance = get_object_manager()
            .read()
            .map_err(|_| {
                ThingCreationError::CreationFailed("ObjectManager lock poisoned".to_string())
            })?
            .get_object(object_id)
            .ok_or_else(|| {
                ThingCreationError::CreationFailed("Created object not found".to_string())
            })?;

        let base = instance
            .read()
            .map_err(|_| {
                ThingCreationError::CreationFailed("GameObjectInstance lock poisoned".to_string())
            })?
            .base();

        let mask = GameLogicStatusMask::from_bits_truncate(status_bits as u64);
        if !mask.is_empty() {
            if let Ok(mut guard) = base.write() {
                guard.set_status(mask, true);
            }
        }

        Ok(Box::new(CommonObjectHandle { object: base }))
    }
}

struct GameLogicDrawableCreator;

impl DrawableCreator for GameLogicDrawableCreator {
    fn create_drawable(
        &self,
        template: &ThingTemplate,
        _status_bits: DrawableStatus,
    ) -> Result<Box<dyn engine_module::Drawable>, ThingCreationError> {
        let Some(client) = TheGameClient::get() else {
            return Err(ThingCreationError::CreationFailed(
                "TheGameClient unavailable".to_string(),
            ));
        };
        let adapter =
            TheThingFactory::find_template(template.get_name().as_str()).ok_or_else(|| {
                ThingCreationError::CreationFailed("Template adapter unavailable".to_string())
            })?;
        let drawable_id = client.create_drawable(adapter.as_ref());
        Ok(Box::new(CommonDrawableHandle { _id: drawable_id }))
    }
}

/// Install the Common ThingFactory -> GameLogic/GameClient bridge.
pub fn install_thing_factory_bridge() {
    set_object_creator(Some(Arc::new(GameLogicObjectCreator)));
    set_drawable_creator(Some(Arc::new(GameLogicDrawableCreator)));
}
