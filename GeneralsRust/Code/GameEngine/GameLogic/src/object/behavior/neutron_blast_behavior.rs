//! NeutronBlastBehavior - Rust conversion of C++ NeutronBlastBehavior
//!
//! Creates a neutron blast when the object dies:
//! - Kills infantry instantly
//! - Kills contained passengers
//! - Makes vehicles unmanned (or destroys combat bikes)
//! - Optionally affects airborne targets and allies

use crate::ai::CommandSourceType;
use crate::common::{
    Bool, KindOf, ModuleData, Real, Relationship, UnsignedInt, XferVersion, PLAYERMASK_ALL,
};
use crate::damage::DamageInfo;
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::draw::TerrainDecalType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::DrawableArcExt;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct NeutronBlastBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub blast_radius: Real,
    pub is_affect_airborne: Bool,
    pub affect_allies: Bool,
}

impl Default for NeutronBlastBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            blast_radius: 10.0,
            is_affect_airborne: true,
            affect_allies: true,
        }
    }
}

crate::impl_behavior_module_data_via_base!(NeutronBlastBehaviorModuleData, base);

impl NeutronBlastBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, NEUTRON_BLAST_BEHAVIOR_FIELDS)
    }
}

fn first_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const NEUTRON_BLAST_BEHAVIOR_FIELDS: &[FieldParse<NeutronBlastBehaviorModuleData>] = &[
    FieldParse {
        token: "BlastRadius",
        parse: |_, data, tokens| {
            data.blast_radius = INI::parse_real(first_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "AffectAirborne",
        parse: |_, data, tokens| {
            data.is_affect_airborne = INI::parse_bool(first_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "AffectAllies",
        parse: |_, data, tokens| {
            data.affect_allies = INI::parse_bool(first_value(tokens)?)?;
            Ok(())
        },
    },
];

pub struct NeutronBlastBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<NeutronBlastBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
}

impl NeutronBlastBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<NeutronBlastBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
        })
    }

    fn neutron_blast_to_object(
        &self,
        source_arc: &Arc<RwLock<GameObject>>,
        target_arc: &Arc<RwLock<GameObject>>,
    ) {
        let Ok(mut target) = target_arc.write() else {
            return;
        };

        if target.is_effectively_dead() {
            return;
        }

        if !self.module_data.affect_allies {
            let Ok(source) = source_arc.read() else {
                return;
            };
            if matches!(source.relationship_to(&target), Relationship::Allies) {
                return;
            }
        }

        if target.is_kind_of(KindOf::Infantry) {
            target.kill(None, None);
        }

        if let Some(contain) = target.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                for contained_id in contain_guard.get_contained_objects() {
                    let _ = OBJECT_REGISTRY.with_object_mut(*contained_id, |contained| {
                        contained.kill(None, None);
                    });
                }
            }
        }

        if target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Drone) {
            if target.is_kind_of(KindOf::CliffJumper) {
                target.kill(None, None);
                return;
            }

            target.set_disabled_unmanned();

            if let Some(ai) = target.get_ai() {
                ai.ai_idle(CommandSourceType::FromAi);
            }

            let _ = TheGameLogic::deselect_object(&*target, PLAYERMASK_ALL, true);

            if let Some(drawable) = target.get_drawable() {
                drawable.set_terrain_decal(TerrainDecalType::None);
            }

            target.set_team_to_neutral();
        }
    }
}

impl UpdateModuleInterface for NeutronBlastBehavior {
    fn update_simple(&mut self) -> UpdateSleepTime {
        UpdateSleepTime::Forever
    }
}

impl DieModuleInterface for NeutronBlastBehavior {
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(source_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let Ok(source) = source_arc.read() else {
            return Ok(());
        };

        let source_id = source.get_id();
        let source_pos = *source.get_position();
        let source_off_map = source.is_off_map();
        let hit_air = self.module_data.is_affect_airborne;
        drop(source);

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(());
        };

        let candidates = partition.get_objects_in_range(&source_pos, self.module_data.blast_radius);
        for id in candidates {
            if id == source_id {
                continue;
            }
            let Some(target_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(target) = target_arc.read() else {
                continue;
            };
            if target.is_effectively_dead() {
                continue;
            }
            if target.is_off_map() != source_off_map {
                continue;
            }
            if !hit_air && (target.is_kind_of(KindOf::Aircraft) || target.is_airborne_target()) {
                continue;
            }
            drop(target);

            self.neutron_blast_to_object(&source_arc, &target_arc);
        }

        Ok(())
    }
}

impl BehaviorModuleInterface for NeutronBlastBehavior {
    fn get_module_name(&self) -> &'static str {
        "NeutronBlastBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for NeutronBlastBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct NeutronBlastBehaviorFactory;
impl NeutronBlastBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(NeutronBlastBehavior::new(thing, module_data)?))
    }
}
