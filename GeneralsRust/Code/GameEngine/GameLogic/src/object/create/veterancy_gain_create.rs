//! VeterancyGainCreate module - Grants initial veterancy level on creation
//!
//! C++ Source: GameLogic/Object/Create/VeterancyGainCreate.cpp

use std::sync::Arc;

use crate::common::VeterancyLevel;
use crate::experience::ExperienceTracker;
use crate::helpers::TheGameLogic;
use crate::object::create::{CreateModule, CreateModuleData};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType, get_science_store};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, ModuleData, Thing as ThingTrait};

/// Data structure for VeterancyGainCreate module
#[derive(Debug, Clone)]
pub struct VeterancyGainCreateModuleData {
    pub base: CreateModuleData,
    pub starting_level: VeterancyLevel,
    pub science_required: ScienceType,
}

impl Default for VeterancyGainCreateModuleData {
    fn default() -> Self {
        Self {
            base: CreateModuleData::new(),
            starting_level: VeterancyLevel::Regular,
            science_required: SCIENCE_INVALID,
        }
    }
}

impl VeterancyGainCreateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, VETERANCY_GAIN_CREATE_FIELDS)
    }
}

impl ModuleData for VeterancyGainCreateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        ModuleData::set_module_tag_name_key(&mut self.base, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        ModuleData::get_module_tag_name_key(&self.base)
    }
}

impl Snapshotable for VeterancyGainCreateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// VeterancyGainCreate module implementation
#[derive(Debug)]
pub struct VeterancyGainCreate {
    base: CreateModule,
    module_data: game_engine::common::thing::module::ModuleDataHandle,
}

impl VeterancyGainCreate {
    pub fn new(
        thing: Arc<dyn ThingTrait>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Self {
        Self {
            base: CreateModule::new(thing),
            module_data,
        }
    }

    /// VeterancyGainCreate.cpp:56–75 uses the live actor and its controller.
    fn apply_starting_veterancy(&self, object: &mut crate::object::Object) {
        let (science_required, starting_level) = {
            let data = self.module_data.read();
            let data = data
                .as_any()
                .downcast_ref::<VeterancyGainCreateModuleData>()
                .expect("VeterancyGainCreate data type");
            (data.science_required, data.starting_level)
        };
        let science_ok = object.with_controlling_player(|player| {
            science_required == SCIENCE_INVALID || player.has_science(science_required)
        });
        if science_ok != Some(true) {
            return;
        }

        let promotion = object
            .with_experience_tracker_mut(|tracker| {
                if !tracker.is_trainable() {
                    return None;
                }
                tracker
                    .set_min_veterancy_level(
                        starting_level,
                        &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                    )
                    .map(|old_level| (old_level, tracker.get_veterancy_level()))
            })
            .flatten();

        if let Some((old_level, new_level)) = promotion {
            object.on_veterancy_level_changed(old_level, new_level, true);
        }
    }
}
impl CreateInterface for VeterancyGainCreate {
    fn on_create(&self) {
        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };

        if let Ok(mut object) = object_arc.write() {
            self.apply_starting_veterancy(&mut object);
        }
    }

    fn on_create_with_owner(&self, owner: &mut dyn std::any::Any) {
        if let Some(object) = owner.downcast_mut::<crate::object::Object>() {
            self.apply_starting_veterancy(object);
        }
    }

    fn on_build_complete(&self) {
        self.base.on_build_complete();
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for VeterancyGainCreate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

fn parse_starting_level(
    _ini: &mut INI,
    data: &mut VeterancyGainCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let level = match token.trim().to_ascii_uppercase().as_str() {
        "REGULAR" => VeterancyLevel::Regular,
        "VETERAN" => VeterancyLevel::Veteran,
        "ELITE" => VeterancyLevel::Elite,
        "HEROIC" => VeterancyLevel::Heroic,
        _ => return Err(INIError::InvalidData),
    };
    data.starting_level = level;
    Ok(())
}

fn parse_science_required(
    ini: &mut INI,
    data: &mut VeterancyGainCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if let Some(sciences) = ini.receiving_science_names() {
        // CPP INI::parseScience -> scanScience -> friend_lookupScience throws
        // for an unknown authored name; do not invent an ID in the module keys.
        data.science_required = sciences
            .science(token.trim())
            .ok_or(INIError::InvalidData)?;
    } else {
        // Explicit standalone compatibility parser retains its application store.
        let store = get_science_store().ok_or(INIError::InvalidData)?;
        data.science_required = store.get_science_from_internal_name(token.trim());
    }
    Ok(())
}

const VETERANCY_GAIN_CREATE_FIELDS: &[FieldParse<VeterancyGainCreateModuleData>] = &[
    FieldParse {
        token: "StartingLevel",
        parse: parse_starting_level,
    },
    FieldParse {
        token: "ScienceRequired",
        parse: parse_science_required,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::ObjectThingHandle;
    use crate::object::update::ai_update::dozer_ai_update::construction_callback_tests::CompletionFixture;

    #[test]
    fn veterancy_create_promotes_the_already_borrowed_actor() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::veterancy_create_promotes_the_already_borrowed_actor"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_BORROWED_VETERANCY_CREATE_CHILD",
        ) {
            return;
        }

        let _isolation = crate::object::registry::test_isolation_lock()
            .lock()
            .unwrap();
        let fixture = CompletionFixture::new(0);
        let thing = Arc::new(ObjectThingHandle::new(&fixture.structure));
        let mut data = VeterancyGainCreateModuleData::default();
        data.starting_level = VeterancyLevel::Elite;
        let create = VeterancyGainCreate::new(
            thing,
            game_engine::common::thing::module::ModuleDataHandle::new(data),
        );
        let mut actor = fixture.structure.write().unwrap();
        actor.attach_experience_tracker_for_test(true);
        assert_eq!(actor.get_veterancy_level(), VeterancyLevel::Regular);
        assert!(actor.get_controlling_player().is_some());
        create.on_create_with_owner(&mut *actor);
        assert_eq!(actor.get_veterancy_level(), VeterancyLevel::Elite);
    }
}
