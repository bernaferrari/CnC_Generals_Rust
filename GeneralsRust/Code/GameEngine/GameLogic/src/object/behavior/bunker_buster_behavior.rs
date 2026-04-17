//! Bunker Buster Behavior Module
//!
//! Behavior module for Bunker Buster weapons that kill garrisoned objects.
//! Handles upgrade checking, crash effects, and bunker-busting on impact.
//!
//! Originally from: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp
//! Original Author: Mark Lorenzen, June 2003
//! Rust port: 2025

use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

use crate::common::{
    AsciiString, Bool, Coord3D, ModuleData, ObjectID, ObjectStatusTypes, PlayerMaskType, Real,
    UnsignedInt, XferVersion,
};
use crate::effects::FXList;
use crate::helpers::{TheFXListStore, TheGameLogic};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use crate::upgrade::template::UpgradeTemplate;
use crate::weapon::WeaponTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as ThingModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};

/// Module data for bunker buster behavior
#[derive(Clone, Debug)]
pub struct BunkerBusterBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Upgrade required for bunker buster to be active (optional)
    pub upgrade_required: Option<AsciiString>,
    /// FX to play on detonation
    pub detonation_fx: Option<Arc<FXList>>,
    /// FX to play while crashing through bunker
    pub crash_through_bunker_fx: Option<Arc<FXList>>,
    /// Frequency (in frames) for crash FX
    pub crash_through_bunker_fx_frequency: UnsignedInt,
    /// Radius of seismic effect
    pub seismic_effect_radius: Real,
    /// Magnitude of seismic effect
    pub seismic_effect_magnitude: Real,
    /// Shockwave weapon template
    pub shockwave_weapon_template: Option<Arc<WeaponTemplate>>,
    /// Occupant damage weapon template
    pub occupant_damage_weapon_template: Option<Arc<WeaponTemplate>>,
}

impl Default for BunkerBusterBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            upgrade_required: None,
            detonation_fx: None,
            crash_through_bunker_fx: None,
            crash_through_bunker_fx_frequency: 4,
            seismic_effect_radius: 140.0,
            seismic_effect_magnitude: 6.0,
            shockwave_weapon_template: None,
            occupant_damage_weapon_template: None,
        }
    }
}

crate::impl_behavior_module_data_via_base!(BunkerBusterBehaviorModuleData, base);

// -------------------------------------------------------------------------------------------------
// INI parsing helpers
// -------------------------------------------------------------------------------------------------

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_ascii_string_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(AsciiString::from(value));
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(value)?);
    Ok(())
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(value)?);
    Ok(())
}

fn parse_fx_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Option<Arc<FXList>>),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(TheFXListStore::find_fx_list(value));
    Ok(())
}

fn parse_weapon_template_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Option<Arc<WeaponTemplate>>),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    let template =
        crate::weapon::with_weapon_store(|store| store.find_weapon_template(value).cloned())
            .ok()
            .flatten();
    setter(template);
    Ok(())
}

const BUNKER_BUSTER_FIELDS: &[FieldParse<BunkerBusterBehaviorModuleData>] = &[
    FieldParse {
        token: "UpgradeRequired",
        parse: |ini, data, tokens| {
            parse_ascii_string_field(ini, &mut |v| data.upgrade_required = Some(v), tokens)
        },
    },
    FieldParse {
        token: "DetonationFX",
        parse: |ini, data, tokens| parse_fx_field(ini, &mut |v| data.detonation_fx = v, tokens),
    },
    FieldParse {
        token: "CrashThroughBunkerFX",
        parse: |ini, data, tokens| {
            parse_fx_field(ini, &mut |v| data.crash_through_bunker_fx = v, tokens)
        },
    },
    FieldParse {
        token: "CrashThroughBunkerFXFrequency",
        parse: |ini, data, tokens| {
            parse_duration_field(
                ini,
                &mut |v| data.crash_through_bunker_fx_frequency = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "SeismicEffectRadius",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.seismic_effect_radius = v, tokens)
        },
    },
    FieldParse {
        token: "SeismicEffectMagnitude",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.seismic_effect_magnitude = v, tokens)
        },
    },
    FieldParse {
        token: "ShockwaveWeaponTemplate",
        parse: |ini, data, tokens| {
            parse_weapon_template_field(ini, &mut |v| data.shockwave_weapon_template = v, tokens)
        },
    },
    FieldParse {
        token: "OccupantDamageWeaponTemplate",
        parse: |ini, data, tokens| {
            parse_weapon_template_field(
                ini,
                &mut |v| data.occupant_damage_weapon_template = v,
                tokens,
            )
        },
    },
];

impl BunkerBusterBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BUNKER_BUSTER_FIELDS)
    }
}

/// Bunker buster behavior module
///
/// Matches C++ BunkerBusterBehavior.cpp implementation:
/// - Tracks target victim
/// - Plays crash FX while missile is killing self
/// - On death, busts the bunker and kills/damages all contained units
/// - Optionally requires upgrade to be active
/// - Supports shockwave weapons and seismic effects
pub struct BunkerBusterBehavior {
    /// Weak reference to owning object
    object: Weak<RwLock<GameObject>>,
    /// Module data
    module_data: Arc<BunkerBusterBehaviorModuleData>,
    /// ID of the victim object (target building)
    victim_id: ObjectID,
    /// Cached upgrade template pointer (would be resolved from upgrade_required name)
    upgrade_required_resolved: Option<Arc<UpgradeTemplate>>,
}

impl BunkerBusterBehavior {
    /// Create new bunker buster behavior
    /// Matches C++ BunkerBusterBehavior::BunkerBusterBehavior
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
        .downcast_ref::<BunkerBusterBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            victim_id: OBJECT_INVALID_ID,
            upgrade_required_resolved: None,
        })
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<BunkerBusterBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "BunkerBusterBehavior requires an owning object".to_string())?;
        let object_id = module_object.get_object_id();
        let object = crate::object::registry::OBJECT_REGISTRY
            .get_object(object_id)
            .ok_or_else(|| format!("BunkerBusterBehavior missing object {}", object_id))?;
        Ok(Self {
            object: Arc::downgrade(&object),
            module_data,
            victim_id: OBJECT_INVALID_ID,
            upgrade_required_resolved: None,
        })
    }

    /// Get module data
    fn get_module_data(&self) -> &BunkerBusterBehaviorModuleData {
        &self.module_data
    }

    /// On object created - resolve upgrade names to pointers
    /// Matches C++ BunkerBusterBehavior::onObjectCreated
    fn on_object_created(&mut self) {
        if let Some(upgrade_name) = &self.module_data.upgrade_required {
            let upgrade = crate::upgrade::center::get_upgrade_center();
            if let Ok(center) = upgrade.read() {
                self.upgrade_required_resolved = center.find_upgrade(upgrade_name.as_str());
            };
        }
    }

    /// Bust the bunker - kill all garrisoned units
    /// Matches C++ BunkerBusterBehavior::bustTheBunker (line 160)
    fn bust_the_bunker(&mut self) {
        let data = self.get_module_data();
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        // Check if upgrade is required and active
        if let Some(upgrade) = &self.upgrade_required_resolved {
            let Some(player_arc) = object_guard.get_controlling_player() else {
                return;
            };
            if let Ok(player_guard) = player_arc.read() {
                if !player_guard.has_upgrade_complete(upgrade) {
                    return;
                }
            };
        }

        // Find the target object
        let target_arc = if self.victim_id != OBJECT_INVALID_ID {
            TheGameLogic::find_object_by_id(self.victim_id)
        } else {
            None
        };
        let target_exists = target_arc.is_some();
        let object_for_fx = target_arc.clone().unwrap_or_else(|| object_arc.clone());

        if target_exists {
            if let Some(target_arc) = target_arc.as_ref() {
                if let Ok(target_guard) = target_arc.read() {
                    if let Some(contain_handle) = target_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain_handle.lock() {
                            if contain_guard.is_bustable() {
                                let source_player_mask = object_guard
                                    .get_controlling_player()
                                    .and_then(|player| {
                                        player.read().ok().map(|p| p.get_player_mask())
                                    })
                                    .unwrap_or_else(PlayerMaskType::none);

                                if let Some(weapon_template) =
                                    data.occupant_damage_weapon_template.as_ref()
                                {
                                    let mut damage_info = crate::damage::DamageInfo::with_simple(
                                        100.0,
                                        object_guard.get_id(),
                                        crate::damage::DamageType::from_u32(
                                            weapon_template.damage_type as u32,
                                        ),
                                        crate::damage::DeathType::from_u32(
                                            weapon_template.death_type as u32,
                                        ),
                                    );
                                    damage_info.input.source_player_mask = source_player_mask;
                                    damage_info.sync_from_input();
                                    let _ = contain_guard
                                        .harm_and_force_exit_all_contained(&mut damage_info);
                                } else {
                                    let _ = contain_guard.kill_all_contained();
                                }
                            }
                        }
                    }
                }
            }
        }

        // Play detonation FX
        if let Some(fx) = &data.detonation_fx {
            let _ = fx.do_fx_obj(&object_for_fx, None);
        }

        // Add seismic simulation (if DO_SEISMIC_SIMULATIONS is defined)
        // SeismicSimulationNode sim(
        //   objectForFX->getPosition(),
        //   modData->m_seismicEffectRadius,
        //   modData->m_seismicEffectMagnitude,
        //   &bunkerBusterHeavingEarthSeismicFilter );
        // TheTerrainVisual->addSeismicSimulation( sim );

        // Fire shockwave weapon
        if let Some(weapon_template) = &data.shockwave_weapon_template {
            if let Ok(obj_guard) = object_for_fx.read() {
                let position = *obj_guard.get_position();
                let _ = crate::weapon::with_weapon_store(|store| {
                    store.create_and_fire_temp_weapon(
                        weapon_template,
                        obj_guard.get_id(),
                        None,
                        Some(&position),
                    )
                });
            }
        }
    }

    /// Get current game frame
    fn get_current_frame(&self) -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    /// Check if object has an AI update interface
    fn has_ai(&self) -> Bool {
        let Some(object) = self.object.upgrade() else {
            return false;
        };
        let Ok(guard) = object.read() else {
            return false;
        };
        guard.get_ai().is_some()
    }

    /// Check if object has missile killing self status
    fn test_status_missile_killing_self(&self) -> Bool {
        let Some(object) = self.object.upgrade() else {
            return false;
        };
        let Ok(guard) = object.read() else {
            return false;
        };
        guard.test_status(ObjectStatusTypes::MissileKillingSelf)
    }

    /// Get current victim from AI
    fn get_current_victim(&self) -> Option<ObjectID> {
        let object = self.object.upgrade()?;
        let guard = object.read().ok()?;
        let ai = guard.get_ai()?;
        let ai_guard = ai.lock().ok()?;
        ai_guard.get_current_victim()
    }

    pub fn crc(
        &self,
        _xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    pub fn xfer(
        &mut self,
        xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl UpdateModuleInterface for BunkerBusterBehavior {
    /// Update callback
    /// Matches C++ BunkerBusterBehavior::update (line 111)
    fn update_simple(&mut self) -> UpdateSleepTime {
        let crash_frequency = self.module_data.crash_through_bunker_fx_frequency;
        let crash_fx = self.module_data.crash_through_bunker_fx.clone();

        // Check if this is a SMART bomb (has AI)
        // AIUpdateInterface *ai = getObject()->getAI();
        if self.has_ai() {
            if self.victim_id == OBJECT_INVALID_ID {
                // Get current victim from AI
                if let Some(victim_id) = self.get_current_victim() {
                    self.victim_id = victim_id;
                }

                // In C++: DEBUG_ASSERTCRASH( victim, ("BunkerBusterBehavior::update... AIUpdateInterface reports no victim." ) );
            }

            // Play crash FX periodically (not too much)
            // if ( TheGameLogic->getFrame() % modData->m_crashThroughBunkerFXFrequency == 1 )
            let current_frame = self.get_current_frame();
            if crash_frequency > 0 && current_frame % crash_frequency == 1 {
                // const FXList *crashFX = modData->m_crashThroughBunkerFX;
                // if ( getObject()->testStatus( OBJECT_STATUS_MISSILE_KILLING_SELF ) && crashFX )
                if self.test_status_missile_killing_self() {
                    if let (Some(fx), Some(object_arc)) = (crash_fx.as_ref(), self.object.upgrade())
                    {
                        let _ = fx.do_fx_obj(&object_arc, None);
                    }
                }
            }
        }

        // Never sleep - always active
        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for BunkerBusterBehavior {
    fn get_module_name(&self) -> &'static str {
        "BunkerBusterBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    /// On object created callback
    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_object_created();
        Ok(())
    }

    /// Death callback - this is where the actual bunker busting happens
    /// Matches C++ BunkerBusterBehavior::onDie (line 147)
    fn on_die(
        &mut self,
        _damage_info: &crate::common::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Do what we came here to do!
        self.bust_the_bunker();
        Ok(())
    }
}

/// Factory for creating bunker buster behaviors
pub struct BunkerBusterBehaviorFactory;

impl BunkerBusterBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(BunkerBusterBehavior::new(thing, module_data)?))
    }
}

/// Glue object that exposes BunkerBusterBehavior through the shared Module trait.
pub struct BunkerBusterBehaviorModule {
    behavior: BunkerBusterBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<BunkerBusterBehaviorModuleData>,
}

impl BunkerBusterBehaviorModule {
    pub fn new(
        behavior: BunkerBusterBehavior,
        module_name: &AsciiString,
        module_data: Arc<BunkerBusterBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &BunkerBusterBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut BunkerBusterBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for BunkerBusterBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer).map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer).map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

impl EngineModule for BunkerBusterBehaviorModule {

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ThingModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {}

    fn on_delete(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> BunkerBusterBehaviorModuleData {
        BunkerBusterBehaviorModuleData {
            base: BehaviorModuleData::default(),
            upgrade_required: Some(AsciiString::from("Upgrade_BunkerBuster")),
            detonation_fx: Some(Arc::new(FXList::new("FX_BunkerBusterDetonation"))),
            crash_through_bunker_fx: Some(Arc::new(FXList::new("FX_CrashThroughBunker"))),
            crash_through_bunker_fx_frequency: 4,
            seismic_effect_radius: 140.0,
            seismic_effect_magnitude: 6.0,
            shockwave_weapon_template: None,
            occupant_damage_weapon_template: None,
        }
    }

    #[test]
    fn test_module_data_defaults() {
        let data = BunkerBusterBehaviorModuleData::default();
        assert_eq!(data.crash_through_bunker_fx_frequency, 4);
        assert_eq!(data.seismic_effect_radius, 140.0);
        assert_eq!(data.seismic_effect_magnitude, 6.0);
        assert!(data.upgrade_required.is_none());
    }

    #[test]
    fn test_module_data_custom() {
        let data = create_test_data();
        assert_eq!(data.crash_through_bunker_fx_frequency, 4);
        assert!(data.upgrade_required.is_some());
        assert!(data.detonation_fx.is_some());
    }
}
