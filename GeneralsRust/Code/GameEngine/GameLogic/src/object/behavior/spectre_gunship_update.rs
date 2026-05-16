//! SpectreGunshipUpdate - Rust conversion of C++ SpectreGunshipUpdate
//! Handles Spectre gunship orbit, targeting, and weapon firing for the special power.

use crate::ai::CommandSourceType;
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::audio::AudioEventRts;
use crate::common::types::Relationship;
use crate::common::types::SHADOW_NAMES;
use crate::common::xfer::XferExt;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::{
    AsciiString, Bool, Coord3D, CoordOrigin, DisabledMaskType, DisabledType, GameLogicRandomValue,
    KindOf, LocomotorSetType, ModuleData, ObjectID, ObjectShroudStatus, ObjectStatusTypes,
    RadiusDecal, RadiusDecalTemplate, Real, UnsignedInt, MODELCONDITION_DOOR_1_CLOSING,
    MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_JETAFTERBURNER,
};
use crate::helpers::TheAudio;
use crate::helpers::{
    TheGameLogic, TheParticleSystemManager, ThePartitionManager, TheTerrainLogic,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, ContainModuleInterfaceExt,
    SpecialPowerCommandOptions, SpecialPowerModuleInterface, SpecialPowerUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::special_power_module::Waypoint;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::update::does_special_power_update_pass_science_test_for_object;
use crate::object::DrawableArcExt;
use crate::object::Object as GameObject;
use crate::object::SpecialPowerTemplate;
use crate::player::PlayerType;
use crate::player::ThePlayerList;
use crate::weapon::with_weapon_store;
use crate::weapon::WeaponTemplate;
use crate::GameLogicRandomValueReal;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const ORBIT_INSERTION_SLOPE_MAX: Real = 0.8;
const ORBIT_INSERTION_SLOPE_MIN: Real = 0.5;
const LOTS_OF_SHOTS: i32 = 9999;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GunshipStatus {
    Inserting,
    Orbiting,
    Departing,
    Idle,
}

/// INI-configurable data for SpectreGunshipUpdate
#[derive(Clone, Debug)]
pub struct SpectreGunshipUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template: Option<Arc<SpecialPowerTemplate>>,
    pub howitzer_weapon_template: Option<Arc<WeaponTemplate>>,
    pub gattling_template_name: AsciiString,
    pub attack_area_decal_template: RadiusDecalTemplate,
    pub targeting_reticle_decal_template: RadiusDecalTemplate,
    pub orbit_frames: UnsignedInt,
    pub howitzer_firing_rate: UnsignedInt,
    pub howitzer_follow_lag: UnsignedInt,
    pub attack_area_radius: Real,
    pub targeting_reticle_radius: Real,
    pub gunship_orbit_radius: Real,
    pub strafing_increment: Real,
    pub orbit_insertion_slope: Real,
    pub random_offset_for_howitzer: Real,
    pub gattling_strafe_fx_particle_system: AsciiString,
}

impl Default for SpectreGunshipUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            howitzer_weapon_template: None,
            gattling_template_name: AsciiString::new(),
            attack_area_decal_template: RadiusDecalTemplate::default(),
            targeting_reticle_decal_template: RadiusDecalTemplate::default(),
            orbit_frames: 0,
            howitzer_firing_rate: 10,
            howitzer_follow_lag: 0,
            attack_area_radius: 200.0,
            targeting_reticle_radius: 25.0,
            gunship_orbit_radius: 250.0,
            strafing_increment: 20.0,
            orbit_insertion_slope: 0.7,
            random_offset_for_howitzer: 20.0,
            gattling_strafe_fx_particle_system: AsciiString::new(),
        }
    }
}

impl SpectreGunshipUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECTRE_GUNSHIP_UPDATE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(SpectreGunshipUpdateModuleData, base);

fn parse_special_power_template(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let name = AsciiString::from(tokens[0]);
    data.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_ascii_string_field(
    _ini: &mut INI,
    target: &mut AsciiString,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    *target = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    tokens[0].parse().map_err(|_| INIError::InvalidData)
}

fn parse_howitzer_firing_rate(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.howitzer_firing_rate = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_orbit_time(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.orbit_frames = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_howitzer_follow_lag(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.howitzer_follow_lag = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_attack_area_radius(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attack_area_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_strafing_increment(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.strafing_increment = parse_real(tokens)?;
    Ok(())
}

fn parse_orbit_insertion_slope(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.orbit_insertion_slope = parse_real(tokens)?;
    Ok(())
}

fn parse_random_offset_for_howitzer(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.random_offset_for_howitzer = parse_real(tokens)?;
    Ok(())
}

fn parse_targeting_reticle_radius(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.targeting_reticle_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_gunship_orbit_radius(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.gunship_orbit_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_howitzer_weapon_template(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.howitzer_weapon_template = None;
        return Ok(());
    }
    let template =
        with_weapon_store(|weapon_store| weapon_store.find_weapon_template(token).cloned())
            .ok()
            .flatten();
    data.howitzer_weapon_template = template;
    Ok(())
}

fn parse_gattling_template_name(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ascii_string_field(_ini, &mut data.gattling_template_name, tokens)
}

fn parse_gattling_strafe_fx_particle_system(
    _ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.gattling_strafe_fx_particle_system = AsciiString::new();
        return Ok(());
    }
    data.gattling_strafe_fx_particle_system = AsciiString::from(*token);
    Ok(())
}

fn parse_radius_decal_texture(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.texture_name = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_radius_decal_style(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.shadow_type = INI::parse_bit_string_32(tokens, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_radius_decal_opacity_min(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.min_opacity = INI::parse_percent_to_real(tokens[0])?;
    data.opacity = data.min_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_max(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.max_opacity = INI::parse_percent_to_real(tokens[0])?;
    data.opacity = data.max_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_throb_time(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.opacity_throb_time = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_radius_decal_color(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens.len() == 1 {
        if let Ok(value) = tokens[0].parse::<u32>() {
            data.color = value;
            return Ok(());
        }
    }

    let mut r: u8 = 0;
    let mut g: u8 = 0;
    let mut b: u8 = 0;
    let mut a: u8 = 255;

    for token in tokens {
        let (key, value) = match token.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => ("", token.trim()),
        };
        let parsed = value.parse::<u8>().map_err(|_| INIError::InvalidData)?;
        match key.to_ascii_uppercase().as_str() {
            "R" => r = parsed,
            "G" => g = parsed,
            "B" => b = parsed,
            "A" => a = parsed,
            _ => {}
        }
    }

    data.color = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
    Ok(())
}

fn parse_radius_decal_only_visible(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.only_visible_to_owning_player = INI::parse_bool(token)?;
    Ok(())
}

const RADIUS_DECAL_TEMPLATE_FIELDS: &[FieldParse<RadiusDecalTemplate>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_radius_decal_texture,
    },
    FieldParse {
        token: "Style",
        parse: parse_radius_decal_style,
    },
    FieldParse {
        token: "OpacityMin",
        parse: parse_radius_decal_opacity_min,
    },
    FieldParse {
        token: "OpacityMax",
        parse: parse_radius_decal_opacity_max,
    },
    FieldParse {
        token: "OpacityThrobTime",
        parse: parse_radius_decal_opacity_throb_time,
    },
    FieldParse {
        token: "Color",
        parse: parse_radius_decal_color,
    },
    FieldParse {
        token: "OnlyVisibleToOwningPlayer",
        parse: parse_radius_decal_only_visible,
    },
];

fn parse_attack_area_decal(
    ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.attack_area_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

fn parse_targeting_reticle_decal(
    ini: &mut INI,
    data: &mut SpectreGunshipUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.targeting_reticle_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

const SPECTRE_GUNSHIP_UPDATE_FIELDS: &[FieldParse<SpectreGunshipUpdateModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template,
    },
    FieldParse {
        token: "GattlingTemplateName",
        parse: parse_gattling_template_name,
    },
    FieldParse {
        token: "HowitzerFiringRate",
        parse: parse_howitzer_firing_rate,
    },
    FieldParse {
        token: "OrbitTime",
        parse: parse_orbit_time,
    },
    FieldParse {
        token: "HowitzerFollowLag",
        parse: parse_howitzer_follow_lag,
    },
    FieldParse {
        token: "AttackAreaRadius",
        parse: parse_attack_area_radius,
    },
    FieldParse {
        token: "StrafingIncrement",
        parse: parse_strafing_increment,
    },
    FieldParse {
        token: "OrbitInsertionSlope",
        parse: parse_orbit_insertion_slope,
    },
    FieldParse {
        token: "RandomOffsetForHowitzer",
        parse: parse_random_offset_for_howitzer,
    },
    FieldParse {
        token: "TargetingReticleRadius",
        parse: parse_targeting_reticle_radius,
    },
    FieldParse {
        token: "GunshipOrbitRadius",
        parse: parse_gunship_orbit_radius,
    },
    FieldParse {
        token: "HowitzerWeaponTemplate",
        parse: parse_howitzer_weapon_template,
    },
    FieldParse {
        token: "GattlingStrafeFXParticleSystem",
        parse: parse_gattling_strafe_fx_particle_system,
    },
    FieldParse {
        token: "AttackAreaDecal",
        parse: parse_attack_area_decal,
    },
    FieldParse {
        token: "TargetingReticleDecal",
        parse: parse_targeting_reticle_decal,
    },
];

pub struct SpectreGunshipUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<SpectreGunshipUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    gattling_id: ObjectID,
    status: GunshipStatus,
    initial_target_position: Coord3D,
    override_target_destination: Coord3D,
    satellite_position: Coord3D,
    gattling_target_position: Coord3D,
    position_to_shoot_at: Coord3D,
    attack_area_decal: Option<RadiusDecal>,
    targeting_reticle_decal: Option<RadiusDecal>,
    orbit_escape_frame: UnsignedInt,
    ok_to_fire_howitzer_counter: UnsignedInt,
    afterburner_sound: AudioEventRts,
    afterburner_handle: Option<u32>,
    howitzer_fire_sound: AudioEventRts,
}

impl SpectreGunshipUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<SpectreGunshipUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            gattling_id: crate::common::INVALID_ID,
            status: GunshipStatus::Idle,
            initial_target_position: Coord3D::ZERO,
            override_target_destination: Coord3D::ZERO,
            satellite_position: Coord3D::ZERO,
            gattling_target_position: Coord3D::ZERO,
            position_to_shoot_at: Coord3D::ZERO,
            attack_area_decal: None,
            targeting_reticle_decal: None,
            orbit_escape_frame: 0,
            ok_to_fire_howitzer_counter: 0,
            afterburner_sound: AudioEventRts::default(),
            afterburner_handle: None,
            howitzer_fire_sound: AudioEventRts::default(),
        })
    }

    fn set_status(&mut self, status: GunshipStatus) {
        self.status = status;
    }

    fn status_before_departing(&self) -> bool {
        (self.status as i32) < (GunshipStatus::Departing as i32)
    }

    fn with_special_power_module<F, R>(&mut self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn SpecialPowerModuleInterface) -> R,
    {
        let mut func = Some(func);
        let obj_arc = self.object.upgrade()?;
        let obj = obj_arc.read().ok()?;
        let template = self.module_data.special_power_template.as_ref()?;
        obj.with_special_power_module_mut_by_name(template.get_name(), |module| {
            let func = func.take().expect("special power callback already used");
            func(module)
        })
    }

    fn is_point_off_map(&self, pos: Coord3D) -> bool {
        let Some(terrain) = TheTerrainLogic::get() else {
            return false;
        };
        let region = terrain.get_extent_including_border();
        let inside = pos.x >= region.lo.x
            && pos.x <= region.hi.x
            && pos.y >= region.lo.y
            && pos.y <= region.hi.y;
        !inside
    }

    fn is_fair_distance_from_ship(&self, target_pos: Coord3D) -> bool {
        let Some(gunship_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(gunship) = gunship_arc.read() else {
            return false;
        };
        let ship_pos = *gunship.get_position();
        let delta = ship_pos - target_pos;
        delta.length() > self.module_data.gunship_orbit_radius * 0.75
    }

    fn is_disguised_as_enemy(&self, target: &GameObject, gunship: &GameObject) -> bool {
        if !target.is_kind_of(KindOf::Disguiser)
            || !target.test_status(ObjectStatusTypes::Disguised)
        {
            return false;
        }

        let mut disguised_player_index = None;
        for behavior in target.get_behavior_modules() {
            let Ok(guard) = behavior.lock() else {
                continue;
            };
            if let Some(idx) = guard.get_disguised_player_index() {
                disguised_player_index = Some(idx);
                break;
            }
        }

        let Some(disguised_index) = disguised_player_index else {
            return false;
        };

        let Some(our_player_arc) = gunship.get_controlling_player() else {
            return false;
        };
        let Ok(our_player) = our_player_arc.read() else {
            return false;
        };

        let list = ThePlayerList().read().ok();
        let other_player_arc = list.as_ref().and_then(|l| l.get_player(disguised_index));
        let Some(other_player_arc) = other_player_arc else {
            return false;
        };
        let Ok(other_player) = other_player_arc.read() else {
            return false;
        };
        let Some(other_team) = other_player.get_default_team() else {
            return false;
        };
        let Ok(other_team_guard) = other_team.read() else {
            return false;
        };

        our_player.get_relationship_with_team(&other_team_guard) == Relationship::Enemies
    }

    fn find_target_in_radius(
        &self,
        gunship: &GameObject,
        center: Coord3D,
        radius: Real,
    ) -> Option<(ObjectID, Coord3D)> {
        let mut best: Option<(ObjectID, Coord3D, Real)> = None;
        let gunship_off_map = gunship.is_off_map();
        let controlling_player_index = if let Some(player_arc) = gunship.get_controlling_player() {
            player_arc
                .read()
                .ok()
                .map(|player| player.get_player_index())
        } else {
            None
        };
        let partition = ThePartitionManager::get()?;
        let radius_sqr = radius * radius;
        for id in partition.get_objects_in_range_boundary_2d(&center, radius) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_effectively_dead() {
                continue;
            }
            if obj.is_off_map() != gunship_off_map {
                continue;
            }
            if obj.is_stealthed() && !obj.is_detected() {
                if !self.is_disguised_as_enemy(&obj, gunship) {
                    continue;
                }
            }
            if gunship.relationship_to(&obj) != Relationship::Enemies {
                continue;
            }
            let can_attack = gunship.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                &obj,
                CommandSourceType::FromAi,
            );
            if !matches!(
                can_attack,
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            ) {
                continue;
            }
            if let Some(player_index) = controlling_player_index {
                let shroud = obj.get_shrouded_status(player_index);
                if shroud != ObjectShroudStatus::Clear {
                    continue;
                }
            }
            let pos = *obj.get_position();
            let target_radius = obj.get_geometry_info().get_bounding_circle_radius();
            if !self.is_fair_distance_from_ship(pos) {
                continue;
            }
            let delta = pos - center;
            let center_dist = delta.length();
            let boundary_dist = if center_dist <= target_radius {
                0.0
            } else {
                center_dist - target_radius
            };
            let dist = boundary_dist * boundary_dist;
            if dist > radius_sqr {
                continue;
            }
            if best.as_ref().map(|b| dist < b.2).unwrap_or(true) {
                best = Some((id, pos, dist));
            }
        }
        best.map(|(id, pos, _)| (id, pos))
    }

    fn can_render_decal(
        template: &RadiusDecalTemplate,
        owner_index: Option<crate::player::PlayerIndex>,
    ) -> bool {
        if !template.only_visible_to_owning_player {
            return true;
        }
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index());
        match (local_index, owner_index) {
            (Some(local), Some(owner)) => local == owner,
            _ => false,
        }
    }

    fn create_decal(
        template: &RadiusDecalTemplate,
        position: Coord3D,
        radius: Real,
        owner_index: Option<crate::player::PlayerIndex>,
    ) -> RadiusDecal {
        if !Self::can_render_decal(template, owner_index) {
            return RadiusDecal::new(Coord3D::origin(), 0.0);
        }
        let mut decal = template.create_radius_decal_with_radius(position, radius);
        if !decal.is_empty() {
            if template.color == 0 {
                if let (Some(owner), Ok(list)) = (owner_index, ThePlayerList().read()) {
                    if let Some(player) = list.get_player(owner).and_then(|p| p.read().ok()) {
                        decal.color = player.get_player_color().to_argb_u32();
                    }
                }
            }
        }
        decal
    }

    fn update_decal_position(decal: &mut Option<RadiusDecal>, position: Coord3D) {
        if let Some(decal) = decal.as_mut() {
            decal.position = position;
        }
    }

    fn clean_up(&mut self) {
        self.attack_area_decal = None;
        self.targeting_reticle_decal = None;

        if self.gattling_id != crate::common::INVALID_ID {
            let _ = TheGameLogic::destroy_object_by_id(self.gattling_id);
        }
    }

    fn disengage_and_depart_ao(&mut self, gunship: &mut GameObject) {
        if let Some(ai) = gunship.get_ai_update_interface() {
            let (dx, dy) = gunship.get_unit_direction_vector_2d();
            let mut exit_point = *gunship.get_position();
            let map_size = 99999.0;
            exit_point.x += dx * map_size;
            exit_point.y += dy * map_size;
            ai.ai_move_to_position(&exit_point, false, CommandSourceType::FromAi);
            ai.choose_locomotor_set(LocomotorSetType::Panic);
            ai.set_allow_invalid_position(true);
            ai.set_ultra_accurate(true);
        }

        if self.gattling_id != crate::common::INVALID_ID {
            if let Some(gattling_arc) = TheGameLogic::find_object_by_id(self.gattling_id) {
                if let Ok(mut gattling) = gattling_arc.write() {
                    gattling.set_disabled(DisabledType::Paralyzed);
                }
            }
        }

        if let Some(draw) = gunship.get_drawable() {
            draw.clear_and_set_model_condition_state(
                MODELCONDITION_DOOR_1_OPENING,
                MODELCONDITION_DOOR_1_CLOSING,
            );
        }

        self.friend_enable_afterburners(gunship, true);
        self.clean_up();
    }

    fn friend_enable_afterburners(&mut self, gunship: &mut GameObject, enable: bool) {
        if enable {
            gunship.set_model_condition_state(MODELCONDITION_JETAFTERBURNER);
            if !self.afterburner_sound.is_currently_playing()
                && !self.afterburner_sound.event_name.is_empty()
            {
                self.afterburner_sound.set_object_id(gunship.get_id());
                if let Some(audio) = TheAudio::get() {
                    let handle = audio.add_audio_event(&self.afterburner_sound);
                    self.afterburner_sound.set_playing_handle(handle);
                    self.afterburner_handle = Some(handle);
                }
            }
        } else {
            gunship.clear_model_condition_state(MODELCONDITION_JETAFTERBURNER);
            if self.afterburner_sound.is_currently_playing() {
                if let Some(audio) = TheAudio::get() {
                    audio.remove_audio_event(self.afterburner_sound.get_playing_handle());
                }
                self.afterburner_sound.set_playing_handle(0);
                self.afterburner_handle = None;
            }
        }
    }
}

impl UpdateModuleInterface for SpectreGunshipUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = TheGameLogic::get_frame();
        let Some(gunship_arc) = self.object.upgrade() else {
            if self.status != GunshipStatus::Idle {
                self.set_status(GunshipStatus::Idle);
                self.clean_up();
            }
            return Ok(UpdateSleepTime::None);
        };

        let Ok(mut gunship) = gunship_arc.write() else {
            if self.status != GunshipStatus::Idle {
                self.set_status(GunshipStatus::Idle);
                self.clean_up();
            }
            return Ok(UpdateSleepTime::None);
        };

        if gunship.is_effectively_dead() {
            return Ok(UpdateSleepTime::Forever);
        }

        if let Some(decal) = self.attack_area_decal.as_mut() {
            decal.update();
        }
        if let Some(decal) = self.targeting_reticle_decal.as_mut() {
            decal.update();
        }

        let gattling_ai = if self.gattling_id != crate::common::INVALID_ID {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(self.gattling_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    obj_guard.get_ai_update_interface()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if matches!(
            self.status,
            GunshipStatus::Inserting | GunshipStatus::Orbiting
        ) {
            let mut perigee = *gunship.get_position() - self.initial_target_position;
            perigee.z = 0.0;
            let distance_to_target = perigee.length();
            let mut perigee_dir = perigee;
            if distance_to_target > 0.0 {
                perigee_dir /= distance_to_target;
            }

            let apogee = Coord3D::new(-perigee_dir.y, perigee_dir.x, 0.0);
            let slope = self
                .module_data
                .orbit_insertion_slope
                .clamp(ORBIT_INSERTION_SLOPE_MIN, ORBIT_INSERTION_SLOPE_MAX);
            let mut declination = perigee_dir * slope + apogee * (1.0 - slope);
            declination.z = 0.0;
            declination *= self.module_data.gunship_orbit_radius;

            self.satellite_position = self.initial_target_position + declination;
            if let Some(ai) = gunship.get_ai_update_interface() {
                ai.ai_move_to_position(&self.satellite_position, false, CommandSourceType::FromAi);
            }

            let constraint_radius =
                self.module_data.attack_area_radius - self.module_data.targeting_reticle_radius;
            let mut override_delta =
                self.initial_target_position - self.override_target_destination;
            if override_delta.length() > constraint_radius {
                override_delta = override_delta.normalize() * constraint_radius;
                self.override_target_destination = self.initial_target_position - override_delta;
            }

            Self::update_decal_position(&mut self.attack_area_decal, self.initial_target_position);
            Self::update_decal_position(
                &mut self.targeting_reticle_decal,
                self.override_target_destination,
            );

            if self.status == GunshipStatus::Inserting
                && distance_to_target < self.module_data.gunship_orbit_radius
            {
                self.set_status(GunshipStatus::Orbiting);
                self.orbit_escape_frame = current_frame + self.module_data.orbit_frames;

                if self.gattling_id != crate::common::INVALID_ID {
                    if let Some(gattling_arc) = TheGameLogic::find_object_by_id(self.gattling_id) {
                        if let Ok(mut gattling) = gattling_arc.write() {
                            gattling.clear_disabled(DisabledType::Paralyzed);
                        }
                    }
                }

                if let Some(draw) = gunship.get_drawable() {
                    draw.clear_and_set_model_condition_state(
                        MODELCONDITION_DOOR_1_CLOSING,
                        MODELCONDITION_DOOR_1_OPENING,
                    );
                }

                if let Some(ai) = gunship.get_ai_update_interface() {
                    ai.choose_locomotor_set(LocomotorSetType::Normal);
                    ai.set_allow_invalid_position(true);
                    ai.set_ultra_accurate(true);
                }
                self.friend_enable_afterburners(&mut gunship, false);
            }
        }

        if self.status == GunshipStatus::Orbiting {
            if current_frame >= self.orbit_escape_frame {
                self.clean_up();
                self.set_status(GunshipStatus::Departing);
                self.disengage_and_depart_ao(&mut gunship);
            } else if self.module_data.howitzer_firing_rate > 0
                && current_frame % self.module_data.howitzer_firing_rate < 1
            {
                let mut _target_pos = self.override_target_destination;
                let mut target_id: Option<ObjectID> = None;
                self.position_to_shoot_at = self.override_target_destination;

                if let Some((id, pos)) = self.find_target_in_radius(
                    &gunship,
                    self.override_target_destination,
                    self.module_data.targeting_reticle_radius,
                ) {
                    target_id = Some(id);
                    _target_pos = pos;
                } else if {
                    if let Some(player_arc) = gunship.get_controlling_player() {
                        player_arc
                            .read()
                            .ok()
                            .map(|player| player.get_player_type() != PlayerType::Human)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                } {
                    if let Some((id, pos)) = self.find_target_in_radius(
                        &gunship,
                        self.initial_target_position,
                        self.module_data.attack_area_radius,
                    ) {
                        target_id = Some(id);
                        _target_pos = pos;
                        self.position_to_shoot_at = _target_pos;
                    }
                }

                if let Some(ai) = gattling_ai.as_ref() {
                    if let Some(target_id) = target_id {
                        ai.ai_attack_object_id(target_id, LOTS_OF_SHOTS, CommandSourceType::FromAi);
                    } else {
                        ai.ai_attack_position(
                            &self.gattling_target_position,
                            LOTS_OF_SHOTS,
                            CommandSourceType::FromAi,
                        );
                    }
                }

                if self.ok_to_fire_howitzer_counter > self.module_data.howitzer_follow_lag {
                    if let Some(template) = self.module_data.howitzer_weapon_template.as_ref() {
                        let offs = self.module_data.random_offset_for_howitzer;
                        let attack_pos = Coord3D::new(
                            self.gattling_target_position.x
                                + GameLogicRandomValueReal!(-offs, offs),
                            self.gattling_target_position.y
                                + GameLogicRandomValueReal!(-offs, offs),
                            self.gattling_target_position.z,
                        );
                        let _ = with_weapon_store(|store| {
                            store.create_and_fire_temp_weapon(
                                template,
                                gunship.get_id(),
                                None,
                                Some(&attack_pos),
                            )
                        });
                        if !self.howitzer_fire_sound.event_name.is_empty() {
                            let mut event = self.howitzer_fire_sound.clone();
                            event.set_object_id(gunship.get_id());
                            if let Some(audio) = TheAudio::get() {
                                audio.add_audio_event(&event);
                            }
                        }
                    }
                }

                let _ = target_id;
            }

            let gattling_firing =
                if let Some(gattling_arc) = TheGameLogic::find_object_by_id(self.gattling_id) {
                    gattling_arc
                        .read()
                        .map(|gattling| gattling.test_status(ObjectStatusTypes::IsFiringWeapon))
                        .unwrap_or(false)
                } else {
                    false
                };

            if gattling_firing
                && !self
                    .module_data
                    .gattling_strafe_fx_particle_system
                    .is_empty()
            {
                let mut delta = self.position_to_shoot_at - self.gattling_target_position;
                let dist = delta.length();
                if dist < self.module_data.strafing_increment {
                    self.gattling_target_position = self.position_to_shoot_at;
                    self.ok_to_fire_howitzer_counter += 1;
                } else {
                    self.ok_to_fire_howitzer_counter = 0;
                    delta = delta.normalize() * self.module_data.strafing_increment;
                    self.gattling_target_position += delta;
                }

                let local_visible = ThePlayerList()
                    .read()
                    .ok()
                    .and_then(|list| {
                        let idx = list.get_local_player_index();
                        if idx >= 0 {
                            Some(idx)
                        } else {
                            None
                        }
                    })
                    .map(|player_idx| {
                        let shroud = gunship.get_shrouded_status(player_idx);
                        (shroud as u8) <= (ObjectShroudStatus::PartialClear as u8)
                    })
                    .unwrap_or(true);

                if local_visible {
                    let mut impact = self.gattling_target_position;
                    impact.x += crate::GameClientRandomValueReal!(-5.0, 5.0);
                    impact.y += crate::GameClientRandomValueReal!(-5.0, 5.0);
                    impact.z = TheTerrainLogic::get()
                        .map(|terrain| terrain.get_ground_height(impact.x, impact.y, None))
                        .unwrap_or(impact.z);
                    if let Some(manager) = TheParticleSystemManager::get() {
                        if let Some(id) = manager.create_particle_system(Some(
                            self.module_data.gattling_strafe_fx_particle_system.as_str(),
                        )) {
                            manager.set_particle_system_position(id, &impact);
                        }
                    }
                }
            }
        } else if self.status == GunshipStatus::Departing {
            if self.is_point_off_map(*gunship.get_position()) {
                let _ = TheGameLogic::destroy_object(&gunship);
                self.set_status(GunshipStatus::Idle);
                self.clean_up();
            }
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

impl SpecialPowerUpdateInterface for SpectreGunshipUpdate {
    fn does_special_power_update_pass_science_test(&self) -> bool {
        let Some(obj_arc) = self.object.upgrade() else {
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

    fn get_extra_required_science(&self) -> crate::common::science::ScienceType {
        crate::common::science::SCIENCE_INVALID
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
            self.override_target_destination = *target_pos;
            self.gattling_target_position = *target_pos;
        } else {
            let now = TheGameLogic::get_frame();
            let _ = self.with_special_power_module(|module| module.set_ready_frame(now));
            self.initial_target_position = *target_pos;
            self.set_status(GunshipStatus::Inserting);
        }

        if let Some(gunship_arc) = self.object.upgrade() {
            if let Ok(mut gunship) = gunship_arc.write() {
                if let Some(ai) = gunship.get_ai_update_interface() {
                    ai.choose_locomotor_set(LocomotorSetType::Panic);
                    ai.set_allow_invalid_position(true);
                    ai.set_ultra_accurate(true);
                }

                let draw = gunship.get_drawable();
                if let Some(draw) = draw {
                    if let Ok(mut draw) = draw.write() {
                        draw.clear_and_set_model_condition_state(
                            MODELCONDITION_DOOR_1_OPENING,
                            MODELCONDITION_DOOR_1_CLOSING,
                        );
                    }
                }

                self.friend_enable_afterburners(&mut gunship, true);
                self.set_status(GunshipStatus::Inserting);

                if let Some(contain) = gunship.get_contain() {
                    if self.gattling_id != crate::common::INVALID_ID
                        && TheGameLogic::find_object_by_id(self.gattling_id).is_some()
                    {
                        self.gattling_id = crate::common::INVALID_ID;
                    }
                    let gattling_template = crate::helpers::TheThingFactory::find_template(
                        self.module_data.gattling_template_name.as_str(),
                    );
                    if let Some(template) = gattling_template {
                        let team = gunship.get_team();
                        if let Some(team) = team {
                            if let Ok(team_guard) = team.read() {
                                if let Ok(factory) = crate::helpers::TheThingFactory::get() {
                                    if let Ok(new_gattling) =
                                        factory.new_object(template, &*team_guard)
                                    {
                                        if let Ok(gattling_read) = new_gattling.read() {
                                            contain.add_to_contain(&gattling_read);
                                            self.gattling_id = gattling_read.get_id();
                                        }
                                        if let Ok(mut gattling) = new_gattling.write() {
                                            gattling.set_disabled(DisabledType::Paralyzed);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let owner_index = gunship
                    .get_controlling_player()
                    .and_then(|player| player.read().ok().map(|p| p.get_player_index()));
                self.attack_area_decal = Some(Self::create_decal(
                    &self.module_data.attack_area_decal_template,
                    *gunship.get_position(),
                    self.module_data.attack_area_radius,
                    owner_index,
                ));
                self.targeting_reticle_decal = Some(Self::create_decal(
                    &self.module_data.targeting_reticle_decal_template,
                    *gunship.get_position(),
                    self.module_data.targeting_reticle_radius,
                    owner_index,
                ));
            }
        }

        let _ = self.with_special_power_module(|module| {
            let location = Coord3D::new(target_pos.x, target_pos.y, target_pos.z);
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
        self.status_before_departing()
    }

    fn get_command_option(&self) -> SpecialPowerCommandOptions {
        SpecialPowerCommandOptions::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        self.status_before_departing()
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        true
    }

    fn set_special_power_overridable_destination(&mut self, location: &Coord3D) {
        if let Some(gunship_arc) = self.object.upgrade() {
            if let Ok(gunship) = gunship_arc.read() {
                if !gunship.is_disabled() {
                    self.override_target_destination = *location;
                    if let Some(controller) = gunship.get_controlling_player() {
                        if let Ok(player) = controller.read() {
                            if player.get_player_index()
                                == ThePlayerList()
                                    .read()
                                    .map(|l| l.get_local_player_index())
                                    .unwrap_or(crate::player::PLAYER_INDEX_INVALID)
                            {
                                let mut sound = gunship.get_template().get_voice_attack();
                                if !sound.event_name.is_empty() {
                                    sound.set_object_id(gunship.get_id());
                                    if let Some(audio) = TheAudio::get() {
                                        audio.add_audio_event(&sound);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

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

impl BehaviorModuleInterface for SpectreGunshipUpdate {
    fn get_module_name(&self) -> &'static str {
        "SpectreGunshipUpdate"
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
        let Some(obj_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let obj = obj_arc.read().ok();
        if let Some(obj) = obj {
            let Some(_template) = &self.module_data.special_power_template else {
                return Err(format!(
                    "SpectreGunshipUpdate missing SpecialPowerTemplate on object {}",
                    obj.get_template().get_name().as_str()
                )
                .into());
            };

            self.satellite_position = *obj.get_position();
            let template = obj.get_template();
            self.afterburner_sound = template
                .get_per_unit_sound("Afterburner")
                .unwrap_or_default();
            self.howitzer_fire_sound = template
                .get_per_unit_sound("HowitzerFire")
                .unwrap_or_default();
        }
        Ok(())
    }
}

impl Snapshotable for SpectreGunshipUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_coord3d(&mut self.initial_target_position);
        xfer.xfer_coord3d(&mut self.override_target_destination);
        xfer.xfer_coord3d(&mut self.satellite_position);

        let mut status = self.status as i32;
        xfer.xfer_i32(&mut status)
            .map_err(|e| format!("Failed to xfer status: {:?}", e))?;
        self.status = match status {
            1 => GunshipStatus::Orbiting,
            2 => GunshipStatus::Departing,
            3 => GunshipStatus::Idle,
            _ => GunshipStatus::Inserting,
        };

        xfer.xfer_unsigned_int(&mut self.orbit_escape_frame)
            .map_err(|e| format!("Failed to xfer orbit_escape_frame: {:?}", e))?;
        if version < 2 {
            let mut attack_decal = RadiusDecal::new(Coord3D::origin(), 0.0);
            let mut targeting_decal = RadiusDecal::new(Coord3D::origin(), 0.0);
            xfer.xfer_radius_decal_mut(&mut attack_decal);
            xfer.xfer_radius_decal_mut(&mut targeting_decal);
            self.attack_area_decal = Some(attack_decal);
            self.targeting_reticle_decal = Some(targeting_decal);
        }
        if version >= 2 {
            xfer.xfer_coord3d(&mut self.gattling_target_position);
            xfer.xfer_coord3d(&mut self.position_to_shoot_at);
            xfer.xfer_unsigned_int(&mut self.ok_to_fire_howitzer_counter)
                .map_err(|e| format!("Failed to xfer ok_to_fire_howitzer_counter: {:?}", e))?;
            xfer.xfer_object_id(&mut self.gattling_id)
                .map_err(|e| format!("Failed to xfer gattling_id: {:?}", e))?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct SpectreGunshipUpdateFactory;

impl SpectreGunshipUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(SpectreGunshipUpdate::new(thing, module_data)?))
    }
}

pub struct SpectreGunshipUpdateModule {
    behavior: SpectreGunshipUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SpectreGunshipUpdateModuleData>,
}

impl SpectreGunshipUpdateModule {
    pub fn new(
        behavior: SpectreGunshipUpdate,
        module_name: &AsciiString,
        module_data: Arc<SpectreGunshipUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpectreGunshipUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SpectreGunshipUpdateModule {
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

impl Module for SpectreGunshipUpdateModule {
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
