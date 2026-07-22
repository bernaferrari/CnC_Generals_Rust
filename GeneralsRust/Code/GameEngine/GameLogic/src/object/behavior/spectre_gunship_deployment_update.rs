//! SpectreGunshipDeploymentUpdate - Rust conversion of C++ SpectreGunshipDeploymentUpdate
//! Spawns and dispatches the Spectre gunship from the command center.

use crate::command_button::CommandButton;
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, DisabledMaskType, ModuleData, ObjectID, ObjectStatusTypes,
    RadiusDecalTemplate, Real, UnsignedInt, XferVersion,
};
use crate::helpers::{TheGameLogic, TheTerrainLogic, TheThingFactory};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, SpecialPowerCommandOptions,
    SpecialPowerModuleInterface, SpecialPowerUpdateInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::special_power_module::Waypoint;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::update::does_special_power_update_pass_science_test_for_object;
use crate::object::Object as GameObject;
use crate::object::SpecialPowerTemplate;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::weapon::WeaponTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::get_science_store;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GunshipCreateLocType {
    CreateAtEdgeNearSource,
    CreateAtEdgeFarthestFromSource,
    CreateAtEdgeNearTarget,
    CreateAtEdgeFarthestFromTarget,
}

#[derive(Clone, Debug)]
pub struct SpectreGunshipDeploymentUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template: Option<Arc<SpecialPowerTemplate>>,
    pub extra_required_science: ScienceType,
    pub howitzer_weapon_template: Option<Arc<WeaponTemplate>>,
    pub gunship_template_name: AsciiString,
    pub gattling_template_name: AsciiString,
    pub attack_area_decal_template: RadiusDecalTemplate,
    pub targeting_reticle_decal_template: RadiusDecalTemplate,
    pub orbit_frames: UnsignedInt,
    pub attack_area_radius: Real,
    pub targeting_reticle_radius: Real,
    pub gunship_orbit_radius: Real,
    pub create_loc: GunshipCreateLocType,
    pub gattling_strafe_fx_particle_system: AsciiString,
}

impl Default for SpectreGunshipDeploymentUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            extra_required_science: SCIENCE_INVALID,
            howitzer_weapon_template: None,
            gunship_template_name: AsciiString::new(),
            gattling_template_name: AsciiString::new(),
            attack_area_decal_template: RadiusDecalTemplate::default(),
            targeting_reticle_decal_template: RadiusDecalTemplate::default(),
            orbit_frames: 0,
            attack_area_radius: 200.0,
            targeting_reticle_radius: 0.0,
            gunship_orbit_radius: 0.0,
            create_loc: GunshipCreateLocType::CreateAtEdgeFarthestFromTarget,
            gattling_strafe_fx_particle_system: AsciiString::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(SpectreGunshipDeploymentUpdateModuleData, base);

impl SpectreGunshipDeploymentUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECTRE_GUNSHIP_DEPLOYMENT_FIELDS)
    }
}

fn parse_ascii_string_field(
    _ini: &mut INI,
    target: &mut AsciiString,
    tokens: &[&str],
) -> Result<(), INIError> {
    *target = AsciiString::from(required_value(tokens)?);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_special_power_template(
    _ini: &mut INI,
    data: &mut SpectreGunshipDeploymentUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = AsciiString::from(required_value(tokens)?);
    data.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_attack_area_radius(
    _ini: &mut INI,
    data: &mut SpectreGunshipDeploymentUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attack_area_radius = INI::parse_real(required_value(tokens)?)?;
    Ok(())
}

fn parse_required_science(
    _ini: &mut INI,
    data: &mut SpectreGunshipDeploymentUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let science = get_science_store()
        .map(|store| store.get_science_from_internal_name(token))
        .unwrap_or(SCIENCE_INVALID);
    data.extra_required_science = science;
    Ok(())
}

fn parse_create_location(
    _ini: &mut INI,
    data: &mut SpectreGunshipDeploymentUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let idx = INI::parse_index_list(required_value(tokens)?.trim(), GUNSHIP_CREATE_LOC_NAMES)?;
    data.create_loc = *GUNSHIP_CREATE_LOC_TYPES
        .get(idx)
        .ok_or(INIError::InvalidData)?;
    Ok(())
}

const GUNSHIP_CREATE_LOC_NAMES: &[&str] = &[
    "CREATE_AT_EDGE_NEAR_SOURCE",
    "CREATE_AT_EDGE_FARTHEST_FROM_SOURCE",
    "CREATE_AT_EDGE_NEAR_TARGET",
    "CREATE_AT_EDGE_FARTHEST_FROM_TARGET",
];

const GUNSHIP_CREATE_LOC_TYPES: &[GunshipCreateLocType] = &[
    GunshipCreateLocType::CreateAtEdgeNearSource,
    GunshipCreateLocType::CreateAtEdgeFarthestFromSource,
    GunshipCreateLocType::CreateAtEdgeNearTarget,
    GunshipCreateLocType::CreateAtEdgeFarthestFromTarget,
];

const SPECTRE_GUNSHIP_DEPLOYMENT_FIELDS: &[FieldParse<SpectreGunshipDeploymentUpdateModuleData>] =
    &[
        FieldParse {
            token: "GunshipTemplateName",
            parse: |ini, data, tokens| {
                parse_ascii_string_field(ini, &mut data.gunship_template_name, tokens)
            },
        },
        FieldParse {
            token: "RequiredScience",
            parse: parse_required_science,
        },
        FieldParse {
            token: "SpecialPowerTemplate",
            parse: parse_special_power_template,
        },
        FieldParse {
            token: "AttackAreaRadius",
            parse: parse_attack_area_radius,
        },
        FieldParse {
            token: "CreateLocation",
            parse: parse_create_location,
        },
    ];

pub struct SpectreGunshipDeploymentUpdate {
    object_id: ObjectID,
    module_data: Arc<SpectreGunshipDeploymentUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    initial_target_position: Coord3D,
    gunship_id: ObjectID,
}

impl SpectreGunshipDeploymentUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<SpectreGunshipDeploymentUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            initial_target_position: Coord3D::ZERO,
            gunship_id: crate::common::INVALID_ID,
        })
    }

    fn with_special_power_module<F, R>(&mut self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn SpecialPowerModuleInterface) -> R,
    {
        let mut func = Some(func);
        let obj_arc = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })?;
        let obj = obj_arc.read().ok()?;
        let template = self.module_data.special_power_template.as_ref()?;
        obj.with_special_power_module_mut_by_name(template.get_name(), |module| {
            let func = func.take().expect("special power callback already used");
            func(module)
        })
    }

    fn compute_creation_point(&self, source: Coord3D, target: Coord3D) -> Coord3D {
        let Some(terrain) = TheTerrainLogic::get() else {
            return source;
        };

        match self.module_data.create_loc {
            GunshipCreateLocType::CreateAtEdgeNearSource => {
                terrain.find_closest_edge_point(&source)
            }
            GunshipCreateLocType::CreateAtEdgeFarthestFromSource => {
                terrain.find_farthest_edge_point(&source)
            }
            GunshipCreateLocType::CreateAtEdgeNearTarget => {
                terrain.find_closest_edge_point(&target)
            }
            GunshipCreateLocType::CreateAtEdgeFarthestFromTarget => {
                terrain.find_farthest_edge_point(&target)
            }
        }
    }
}

impl UpdateModuleInterface for SpectreGunshipDeploymentUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return Ok(UpdateSleepTime::None);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(UpdateSleepTime::None);
        };

        if obj.test_status(ObjectStatusTypes::Sold)
            || obj.is_under_construction()
            || obj.is_effectively_dead()
        {
            return Ok(UpdateSleepTime::Forever);
        }

        Ok(UpdateSleepTime::None)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::DISABLED_SUBDUED
            | DisabledMaskType::DISABLED_UNDERPOWERED
            | DisabledMaskType::DISABLED_EMP
            | DisabledMaskType::DISABLED_HACKED
    }
}

impl SpecialPowerUpdateInterface for SpectreGunshipDeploymentUpdate {
    fn does_special_power_update_pass_science_test(&self) -> bool {
        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return false;
        };
        let Ok(obj) = obj_arc.read() else {
            return false;
        };
        does_special_power_update_pass_science_test_for_object(
            &obj,
            self.get_extra_required_science(),
        )
    }

    fn get_extra_required_science(&self) -> ScienceType {
        self.module_data.extra_required_science
    }

    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        _target_obj: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        _waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> bool {
        let matches_module = self
            .with_special_power_module(|module| module.is_module_for_power(special_power_template))
            .unwrap_or(false);
        if !matches_module {
            return false;
        }

        let Some(target_pos) = target_pos else {
            return false;
        };

        if !command_options.contains(SpecialPowerCommandOptions::COMMAND_FIRED_BY_SCRIPT) {
            self.initial_target_position = *target_pos;
        } else {
            let now = TheGameLogic::get_frame();
            let _ = self.with_special_power_module(|module| module.set_ready_frame(now));
            self.initial_target_position = *target_pos;
        }

        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return false;
        };
        let Ok(obj) = obj_arc.read() else {
            return false;
        };

        if TheGameLogic::find_object_by_id(self.gunship_id).is_some() {
            self.gunship_id = crate::common::INVALID_ID;
        }

        let mut gunship_arc = None;
        if let (Some(gunship_template), Some(team)) = (
            TheThingFactory::find_template(self.module_data.gunship_template_name.as_str()),
            obj.get_team(),
        ) {
            if let (Ok(team_guard), Ok(factory)) = (team.read(), TheThingFactory::get()) {
                if let Ok(new_gunship) = factory.new_object(gunship_template, &*team_guard) {
                    gunship_arc = Some(new_gunship);
                }
            }
        }

        if let Some(gunship_arc) = gunship_arc.as_ref() {
            if let Ok(mut gunship) = gunship_arc.write() {
                gunship.set_producer(Some(&*obj));
                let source_pos = *obj.get_position();
                let mut creation_coord = self.compute_creation_point(source_pos, *target_pos);
                let mut delta = self.initial_target_position - creation_coord;
                let dist = delta.length();
                if dist > 0.0 {
                    delta = delta.normalize() * (dist + self.module_data.gunship_orbit_radius);
                    creation_coord = self.initial_target_position - delta;
                }
                creation_coord.z = gunship
                    .get_ai_update_interface()
                    .and_then(|ai| ai.get_preferred_height())
                    .unwrap_or(0.0);
                let _ = gunship.set_position(&creation_coord);
                let orient = (self.initial_target_position.y - creation_coord.y)
                    .atan2(self.initial_target_position.x - creation_coord.x);
                let _ = gunship.set_orientation(orient);
            }

            self.gunship_id = gunship_arc
                .read()
                .map(|o| o.get_id())
                .unwrap_or(crate::common::INVALID_ID);

            if let Ok(gunship) = gunship_arc.write() {
                let _ = gunship.with_special_power_module_mut_by_name(
                    special_power_template.get_name(),
                    |sp| {
                        let loc = self.initial_target_position;
                        sp.mark_special_power_triggered(Some(&loc));
                        sp.do_special_power_at_location(&loc, INVALID_ANGLE, command_options);
                    },
                );
            }

            if let Some(player_arc) = obj.get_controlling_player() {
                if let Ok(player) = player_arc.read() {
                    if let Ok(gunship) = gunship_arc.read() {
                        let _ = TheGameLogic::select_object(
                            &*gunship,
                            true,
                            player.get_player_mask(),
                            true,
                        );
                    }
                }
            }
        } else {
            self.gunship_id = crate::common::INVALID_ID;
        }

        let location = self.initial_target_position;
        let _ = self.with_special_power_module(|module| {
            module.mark_special_power_triggered(Some(&location));
        });

        true
    }

    fn is_special_ability(&self) -> bool {
        false
    }

    fn is_special_power(&self) -> bool {
        true
    }

    fn is_active(&self) -> bool {
        false
    }

    fn get_command_option(&self) -> SpecialPowerCommandOptions {
        SpecialPowerCommandOptions::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        false
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        false
    }

    fn set_special_power_overridable_destination(&mut self, _location: &Coord3D) {}

    fn is_power_currently_in_use(&self, _command: Option<&CommandButton>) -> bool {
        false
    }

    fn update_special_power(
        &mut self,
        _frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_power_ready(&self) -> bool {
        true
    }
}

impl BehaviorModuleInterface for SpectreGunshipDeploymentUpdate {
    fn get_module_name(&self) -> &'static str {
        "SpectreGunshipDeploymentUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return Ok(());
        };
        let obj = obj_arc.read().ok();
        if let Some(obj) = obj {
            if let Some(template) = &self.module_data.special_power_template {
                let _ = template;
            } else {
                return Err(format!(
                    "SpectreGunshipDeploymentUpdate missing SpecialPowerTemplate on object {}",
                    obj.get_template().get_name().as_str()
                )
                .into());
            }
        }
        Ok(())
    }
}

impl Snapshotable for SpectreGunshipDeploymentUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_object_id(&mut self.gunship_id)
            .map_err(|e| format!("Failed to xfer gunship_id: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct SpectreGunshipDeploymentUpdateFactory;

impl SpectreGunshipDeploymentUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(SpectreGunshipDeploymentUpdate::new(
            thing,
            module_data,
        )?))
    }
}

pub struct SpectreGunshipDeploymentUpdateModule {
    behavior: SpectreGunshipDeploymentUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SpectreGunshipDeploymentUpdateModuleData>,
}

impl SpectreGunshipDeploymentUpdateModule {
    pub fn new(
        behavior: SpectreGunshipDeploymentUpdate,
        module_name: &AsciiString,
        module_data: Arc<SpectreGunshipDeploymentUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpectreGunshipDeploymentUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SpectreGunshipDeploymentUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for SpectreGunshipDeploymentUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectre_deployment_fields_use_cpp_ini_token_handling() {
        let mut data = SpectreGunshipDeploymentUpdateModuleData::default();
        let mut ini = INI::new();

        parse_ascii_string_field(
            &mut ini,
            &mut data.gunship_template_name,
            &["=", "AmericaJetSpectreGunship"],
        )
        .expect("gunship template");
        parse_attack_area_radius(&mut ini, &mut data, &["=", "333.5f"]).expect("attack radius");
        parse_create_location(&mut ini, &mut data, &["=", "CREATE_AT_EDGE_NEAR_SOURCE"])
            .expect("create location");

        assert_eq!(
            data.gunship_template_name.as_str(),
            "AmericaJetSpectreGunship"
        );
        assert!((data.attack_area_radius - 333.5).abs() < f32::EPSILON);
        assert_eq!(
            data.create_loc,
            GunshipCreateLocType::CreateAtEdgeNearSource
        );
    }

    #[test]
    fn spectre_deployment_rejects_missing_values_and_invalid_create_location_like_cpp() {
        let mut data = SpectreGunshipDeploymentUpdateModuleData::default();
        let mut ini = INI::new();

        let err =
            parse_create_location(&mut ini, &mut data, &["NOT_A_LOCATION"]).expect_err("bad loc");
        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(
            data.create_loc,
            GunshipCreateLocType::CreateAtEdgeFarthestFromTarget
        );

        let err = parse_attack_area_radius(&mut ini, &mut data, &["="]).expect_err("missing real");
        assert!(matches!(err, INIError::InvalidData));
    }
}
