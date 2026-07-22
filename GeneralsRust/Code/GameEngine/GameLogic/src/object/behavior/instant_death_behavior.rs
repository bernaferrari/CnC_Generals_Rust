//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp`.

use std::sync::{Arc, RwLock};

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};

use crate::common::{GameLogicRandomValue, ModuleData, TheFXListStore, TheObjectCreationListStore};
use crate::damage::DamageInfo;
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{TheGameLogic, TheWeaponStore};
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels, DieModule, DieModuleData, DieModuleInterface,
};
use crate::object::Object;
use crate::weapon::{with_weapon_store, WeaponTemplate};

#[derive(Debug, Clone)]
pub struct InstantDeathBehaviorModuleData {
    pub base: DieModuleData,
    pub fx: Vec<Arc<FXList>>,
    pub ocls: Vec<Arc<ObjectCreationList>>,
    pub weapons: Vec<Arc<WeaponTemplate>>,
}

impl Default for InstantDeathBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            fx: Vec::new(),
            ocls: Vec::new(),
            weapons: Vec::new(),
        }
    }
}

impl Snapshotable for InstantDeathBehaviorModuleData {
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

crate::impl_legacy_module_data_via_base!(InstantDeathBehaviorModuleData, base);

impl InstantDeathBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, INSTANT_DEATH_BEHAVIOR_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_fx(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(fx) = TheFXListStore::find_fx_list(name) {
                data.fx.push(fx);
            }
        }
    }
    Ok(())
}

fn parse_ocl(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(name) {
                data.ocls.push(ocl);
            }
        }
    }
    Ok(())
}

fn parse_weapon(
    _ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let template = with_weapon_store(|store| store.find_weapon_template(name).cloned())
                .ok()
                .flatten();
            if let Some(weapon) = template {
                data.weapons.push(weapon);
            }
        }
    }
    Ok(())
}

const INSTANT_DEATH_BEHAVIOR_FIELDS: &[FieldParse<InstantDeathBehaviorModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
    FieldParse {
        token: "FX",
        parse: parse_fx,
    },
    FieldParse {
        token: "OCL",
        parse: parse_ocl,
    },
    FieldParse {
        token: "Weapon",
        parse: parse_weapon,
    },
];

#[derive(Debug)]
pub struct InstantDeathBehavior {
    base: DieModule<InstantDeathBehaviorModuleData>,
}

impl InstantDeathBehavior {
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<InstantDeathBehaviorModuleData>,
    ) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    pub fn get_module_name() -> &'static str {
        "InstantDeathBehavior"
    }
}

impl DieModuleInterface for InstantDeathBehavior {
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        if let Some(ai) = object.get_ai_update_interface() {
            let Ok(mut ai_guard) = ai.lock() else {
                return;
            };
            if ai_guard.is_ai_in_dead_state() {
                return;
            }
            ai_guard.mark_as_dead();
        }

        let object_arc = self.base.get_object();

        if let Some(ref object_arc) = object_arc {
            if !self.base.module_data.fx.is_empty() {
                let idx = GameLogicRandomValue(0, self.base.module_data.fx.len() as i32 - 1) as usize;
                if let Some(fx) = self.base.module_data.fx.get(idx) {
                    let _ = fx.do_fx_obj(object_arc, None);
                }
            }

            if !self.base.module_data.ocls.is_empty() {
                let idx = GameLogicRandomValue(0, self.base.module_data.ocls.len() as i32 - 1) as usize;
                if let Some(ocl) = self.base.module_data.ocls.get(idx) {
                    let _ = ObjectCreationList::create(ocl, object_arc, None);
                }
            }
        }

        if !self.base.module_data.weapons.is_empty() {
            let idx =
                GameLogicRandomValue(0, self.base.module_data.weapons.len() as i32 - 1) as usize;
            if let Some(weapon) = self.base.module_data.weapons.get(idx) {
                let position = *object.get_position();
                if let Some(weapon_store) = TheWeaponStore::get() {
                    let _ = weapon_store.create_and_fire_temp_weapon_at_pos(
                        weapon,
                        object.get_id(),
                        &position,
                    );
                }
            }
        }

        let _ = TheGameLogic::destroy_object(object);
    }
}
