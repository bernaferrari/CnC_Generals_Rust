//! GrantUpgradeCreate module - Grants upgrade on object creation/build completion
//!
//! C++ Source: GameLogic/Object/Create/GrantUpgradeCreate.cpp

use std::sync::Arc;

use crate::common::{ObjectStatusMaskType, ObjectStatusTypes};
use crate::helpers::TheGameLogic;
use crate::object::create::{CreateModule, CreateModuleData};
use crate::player::PlayerArcExt;
use crate::upgrade::{center::with_upgrade_center, UpgradeStatus, UpgradeType};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, ModuleData, Thing as ThingTrait};

/// Data structure for GrantUpgradeCreate module
#[derive(Debug, Clone)]
pub struct GrantUpgradeCreateModuleData {
    pub base: CreateModuleData,
    pub upgrade_name: AsciiString,
    pub exempt_status: ObjectStatusMaskType,
}

impl Default for GrantUpgradeCreateModuleData {
    fn default() -> Self {
        Self {
            base: CreateModuleData::new(),
            upgrade_name: AsciiString::new(),
            exempt_status: ObjectStatusMaskType::none(),
        }
    }
}

impl GrantUpgradeCreateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, GRANT_UPGRADE_CREATE_FIELDS)
    }
}

impl ModuleData for GrantUpgradeCreateModuleData {
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

impl Snapshotable for GrantUpgradeCreateModuleData {
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

/// GrantUpgradeCreate module implementation
#[derive(Debug)]
pub struct GrantUpgradeCreate {
    base: CreateModule,
    module_data: Arc<GrantUpgradeCreateModuleData>,
}

impl GrantUpgradeCreate {
    pub fn new(thing: Arc<dyn ThingTrait>, module_data: Arc<GrantUpgradeCreateModuleData>) -> Self {
        Self {
            base: CreateModule::new(thing),
            module_data,
        }
    }

    fn apply_upgrade(&self, record_granted: bool) {
        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        let upgrade = with_upgrade_center(|center| {
            center.find_upgrade(self.module_data.upgrade_name.as_str())
        });
        let Some(upgrade) = upgrade else {
            log::warn!(
                "GrantUpgradeCreate for object {} can't find upgrade template {}",
                object_id,
                self.module_data.upgrade_name
            );
            return;
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Ok(mut obj_guard) = object_arc.write() else {
            return;
        };
        if upgrade.get_upgrade_type() == UpgradeType::Player {
            if let Some(player) = obj_guard.get_controlling_player() {
                player.add_upgrade(&upgrade, UpgradeStatus::Complete);
                if record_granted {
                    if let Ok(mut player_guard) = player.write() {
                        player_guard
                            .get_academy_stats_mut()
                            .record_upgrade(&upgrade, true);
                    }
                }
            }
        } else {
            obj_guard.give_upgrade(&upgrade);
            if record_granted {
                if let Some(player) = obj_guard.get_controlling_player() {
                    if let Ok(mut player_guard) = player.write() {
                        player_guard
                            .get_academy_stats_mut()
                            .record_upgrade(&upgrade, true);
                    }
                }
            }
        }
    }
}

impl CreateInterface for GrantUpgradeCreate {
    fn on_create(&self) {
        let exempt_status = self.module_data.exempt_status;
        if !exempt_status.test(ObjectStatusTypes::UnderConstruction) {
            return;
        }

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
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        if object_guard.test_status(ObjectStatusTypes::UnderConstruction) {
            return;
        }

        drop(object_guard);
        self.apply_upgrade(true);
    }

    fn on_build_complete(&self) {
        if !self.base.should_do_on_build_complete() {
            return;
        }

        self.base.on_build_complete();
        self.apply_upgrade(false);
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for GrantUpgradeCreate {
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

fn parse_upgrade_to_grant(
    _ini: &mut INI,
    data: &mut GrantUpgradeCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_name = AsciiString::from(token.trim());
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut GrantUpgradeCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mask =
        ObjectStatusMaskType::parse_tokens(tokens.iter().skip_while(|t| **t == "=").copied())
            .map_err(|_| INIError::InvalidData)?;
    data.exempt_status = mask;
    Ok(())
}

const GRANT_UPGRADE_CREATE_FIELDS: &[FieldParse<GrantUpgradeCreateModuleData>] = &[
    FieldParse {
        token: "UpgradeToGrant",
        parse: parse_upgrade_to_grant,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_exempt_status,
    },
];
