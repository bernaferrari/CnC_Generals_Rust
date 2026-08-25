//! Canonical leftover WeaponTemplate extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{INVALID_ID, ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion};
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheObjectCreationListStore, TheTerrainLogic, TheThingFactory,
    get_game_logic_random_value, get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_particle_sys::{IniParticleSys, ParticleSystemTemplate};
use game_engine::common::system::Snapshotable;

use super::audio_event::AudioEventRts;
use super::helpers::{
    INVALID_OBJECT_ID, ObjectId, dual_world_registry_unavailable, map_weapon_slot_to_common,
};
use super::masks_enums::*;
use super::store::with_weapon_store;
use super::weapon_instance::Weapon;

/// Weapon template defining weapon properties
#[derive(Debug, Clone)]
pub struct WeaponTemplate {
    /// Basic properties
    pub name: String,
    pub name_key: u32,

    /// Damage properties
    pub primary_damage: f32,
    pub primary_damage_radius: f32,
    pub secondary_damage: f32,
    pub secondary_damage_radius: f32,
    pub shock_wave_amount: f32,
    pub shock_wave_radius: f32,
    pub shock_wave_taper_off: f32,

    /// Range and targeting
    pub attack_range: f32,
    pub minimum_attack_range: f32,
    pub request_assist_range: f32,
    pub aim_delta: f32,
    pub scatter_radius: f32,
    pub scatter_target_scalar: f32,
    pub scatter_targets: Vec<Coord2D>,

    /// Timing and reload
    pub min_delay_between_shots: i32,
    pub max_delay_between_shots: i32,
    pub clip_size: i32,
    pub clip_reload_time: i32,
    pub pre_attack_delay: i32,
    pub auto_reload_when_idle_frames: u32,
    pub suspend_fx_delay: u32,

    /// Weapon behavior
    pub weapon_speed: f32,
    pub min_weapon_speed: f32,
    pub is_scale_weapon_speed: bool,
    pub weapon_recoil: f32,
    pub min_target_pitch: f32,
    pub max_target_pitch: f32,
    pub radius_damage_angle: f32,

    /// Projectile
    pub projectile_name: String,
    pub projectile_stream_name: String,
    pub laser_name: String,
    pub laser_bone_name: String,

    /// Damage and death types
    pub damage_type: DamageType,
    pub damage_status_type: ObjectStatusTypes,
    pub death_type: DeathType,

    /// Masks and flags
    pub anti_mask: WeaponAntiMask,
    pub affects_mask: WeaponAffectsMask,
    pub collide_mask: WeaponCollideMask,

    /// Weapon type properties
    pub damage_dealt_at_self_position: bool,
    pub reload_type: WeaponReloadType,
    pub prefire_type: WeaponPrefireType,
    pub leech_range_weapon: bool,
    pub capable_of_following_waypoint: bool,
    pub is_shows_ammo_pips: bool,
    pub allow_attack_garrisoned_bldgs: bool,
    pub play_fx_when_stealthed: bool,
    pub die_on_detonate: bool,
    pub must_travel_pfx: bool,

    /// Continuous fire
    pub continuous_fire_one_shots_needed: i32,
    pub continuous_fire_two_shots_needed: i32,
    pub continuous_fire_coast_frames: u32,

    /// Special targeting
    pub continue_attack_range: f32,
    pub infantry_inaccuracy_dist: f32,

    /// Barrel management
    pub shots_per_barrel: i32,

    /// Historic bonus
    pub historic_bonus_time: u32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_count: i32,
    pub historic_bonus_weapon: Option<Weak<WeaponTemplate>>,
    /// C++ `INI::parseWeaponTemplate` name kept so fire-time lookup works when
    /// the Weak is not yet wired (Weapon.ini loads HistoricBonusWeapon by name).
    pub historic_bonus_weapon_name: String,

    /// Audio
    pub fire_sound: AudioEventRts,
    pub fire_sound_loop_time: u32,

    /// Per-veterancy level effects (Regular, Veteran, Elite, Heroic)
    pub fire_fx: [Option<FXList>; 4],
    pub projectile_detonate_fx: [Option<FXList>; 4],
    /// Direct OCL handles used by programmatic callers. Weapon.ini references
    /// are retained separately because C++ resolves them after all OCL blocks
    /// have been loaded.
    pub fire_ocl: [Option<Arc<ObjectCreationList>>; 4],
    pub projectile_detonation_ocl: [Option<Arc<ObjectCreationList>>; 4],
    /// Raw C++ `FireOCL` / `VeterancyFireOCL` names, one per veterancy level.
    pub fire_ocl_names: [Option<String>; 4],
    /// Raw C++ `ProjectileDetonationOCL` names, one per veterancy level.
    pub projectile_detonation_ocl_names: [Option<String>; 4],
    /// Direct particle-template handles used by programmatic callers.
    pub projectile_exhaust: [Option<Arc<ParticleSystemTemplate>>; 4],
    /// Raw C++ `ProjectileExhaust` names, resolved lazily once ParticleSystem
    /// definitions are available. This avoids inventing a default template.
    pub projectile_exhaust_names: [Option<String>; 4],

    /// Bonuses
    pub extra_bonus: Option<WeaponBonusSet>,

    /// Historic damage tracking
    historic_damage: Arc<Mutex<VecDeque<HistoricWeaponDamageInfo>>>,

    /// Next template for inheritance
    next_template: Option<Box<WeaponTemplate>>,
}

impl WeaponTemplate {
    /// C++ WeaponTemplate::shouldProjectileCollideWith parity used by projectile behaviors.
    pub fn should_projectile_collide_with(
        &self,
        projectile_launcher: ObjectID,
        projectile: ObjectID,
        thing_we_collided_with: ObjectID,
        intended_victim_id: ObjectID,
    ) -> bool {
        if intended_victim_id != INVALID_ID && thing_we_collided_with == intended_victim_id {
            return true;
        }

        let Some(projectile_obj) = TheGameLogic::find_object_by_id(projectile) else {
            return false;
        };
        let Some(collided_obj) = TheGameLogic::find_object_by_id(thing_we_collided_with) else {
            return false;
        };

        let Ok(projectile_guard) = projectile_obj.read() else {
            return false;
        };
        let Ok(collided_guard) = collided_obj.read() else {
            return false;
        };

        if let Some(launcher_obj) = TheGameLogic::find_object_by_id(projectile_launcher) {
            if let Ok(launcher_guard) = launcher_obj.read() {
                if launcher_guard.get_id() == collided_guard.get_id() {
                    return false;
                }
                if launcher_guard.get_contained_by() == Some(collided_guard.get_id()) {
                    return false;
                }
            }
        }

        if matches!(
            self.damage_type,
            DamageType::Flame | DamageType::ParticleBeam
        ) && collided_guard.test_status(crate::common::ObjectStatusTypes::Burned)
        {
            return false;
        }

        if collided_guard.is_kind_of(KindOf::FSAirfield)
            && intended_victim_id != INVALID_ID
            && collided_guard
                .with_parking_place_behavior(|parking| {
                    parking.has_reserved_space(intended_victim_id)
                })
                .unwrap_or(false)
        {
            return false;
        }

        if let Some(ai) = collided_guard.get_ai() {
            if let Ok(ai_guard) = ai.lock() {
                let mut offset = Coord3D::ZERO;
                if ai_guard.get_sneaky_targeting_offset(&mut offset) {
                    return false;
                }
            }
        }

        let mut required_mask = 0u32;
        match projectile_guard.relationship_to(&collided_guard) {
            Relationship::Allies => required_mask |= WeaponCollideMask::ALLIES,
            Relationship::Enemies => required_mask |= WeaponCollideMask::ENEMIES,
            _ => {}
        }

        if collided_guard.is_kind_of(KindOf::Structure) {
            if collided_guard.get_controlling_player_id()
                == projectile_guard.get_controlling_player_id()
            {
                required_mask |= WeaponCollideMask::CONTROLLED_STRUCTURES;
            } else {
                required_mask |= WeaponCollideMask::STRUCTURES;
            }
        }
        if collided_guard.is_kind_of(KindOf::Shrubbery) {
            required_mask |= WeaponCollideMask::SHRUBBERY;
        }
        if collided_guard.is_kind_of(KindOf::Projectile) {
            required_mask |= WeaponCollideMask::PROJECTILE;
        }
        if collided_guard.is_kind_of(KindOf::Barrier) {
            required_mask |= WeaponCollideMask::WALLS;
        }
        if collided_guard.is_kind_of(KindOf::SmallMissile) {
            required_mask |= WeaponCollideMask::SMALL_MISSILES;
        }
        if collided_guard.is_kind_of(KindOf::BallisticMissile) {
            required_mask |= WeaponCollideMask::BALLISTIC_MISSILES;
        }

        (self.collide_mask.bits() & required_mask) != 0
    }

    fn projectile_template(&self) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let name = self.projectile_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("NONE") {
            return None;
        }
        TheThingFactory::find_template(name)
    }

    #[allow(dead_code)]
    fn projectile_special_power_template(&self) -> Option<String> {
        let template = self.projectile_template()?;
        for info in template.get_behavior_module_info() {
            if info.name.as_str() != "SpecialPowerCompletionDie" {
                continue;
            }
            if let Some(template_name) = info.data.get_special_power_completion_template() {
                return Some(template_name.to_string());
            }
        }
        None
    }

    fn projectile_has_behavior(&self, behavior_name: &str) -> bool {
        let Some(template) = self.projectile_template() else {
            return false;
        };
        template
            .get_behavior_module_info()
            .iter()
            .any(|info| info.name.as_str() == behavior_name)
    }

    fn with_projectile_missile_ai_data<R>(
        &self,
        f: impl FnOnce(&MissileAIUpdateModuleData) -> R,
    ) -> Option<R> {
        let template = self.projectile_template()?;
        for info in template.get_behavior_module_info() {
            if info.name.as_str() != "MissileAIUpdate" {
                continue;
            }
            if let Some(data) = info
                .data
                .as_any()
                .downcast_ref::<MissileAIUpdateModuleData>()
            {
                return Some(f(data));
            }
        }
        None
    }

    fn projectile_missile_fuel_lifetime_seconds(&self) -> Option<crate::common::Real> {
        self.with_projectile_missile_ai_data(|data| {
            if data.fuel_lifetime == 0 {
                None
            } else {
                Some(
                    data.fuel_lifetime as crate::common::Real
                        / LOGICFRAMES_PER_SECOND as crate::common::Real,
                )
            }
        })
        .flatten()
    }

    fn projectile_missile_initial_velocity(&self) -> Option<crate::common::Real> {
        self.with_projectile_missile_ai_data(|data| {
            if data.use_weapon_speed {
                self.weapon_speed.max(self.min_weapon_speed)
            } else if data.initial_velocity > 0.0 {
                data.initial_velocity
            } else {
                self.weapon_speed.max(self.min_weapon_speed)
            }
        })
    }

    fn projectile_missile_homing_delay(&self) -> Option<crate::common::Real> {
        self.with_projectile_missile_ai_data(|data| {
            if data.initial_distance <= 0.0 {
                return 0.0;
            }
            let speed = if data.use_weapon_speed {
                self.weapon_speed.max(self.min_weapon_speed)
            } else if data.initial_velocity > 0.0 {
                data.initial_velocity
            } else {
                self.weapon_speed.max(self.min_weapon_speed)
            };
            if speed <= 0.0 {
                0.0
            } else {
                data.initial_distance / speed
            }
        })
    }

    pub fn new(name: String) -> Self {
        Self {
            name,
            name_key: 0,
            primary_damage: 0.0,
            primary_damage_radius: 0.0,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            attack_range: 0.0,
            minimum_attack_range: 0.0,
            request_assist_range: 0.0,
            aim_delta: 0.0,
            scatter_radius: 0.0,
            scatter_target_scalar: 0.0,
            scatter_targets: Vec::new(),
            min_delay_between_shots: 0,
            max_delay_between_shots: 0,
            clip_size: 0,
            clip_reload_time: 0,
            pre_attack_delay: 0,
            auto_reload_when_idle_frames: 0,
            suspend_fx_delay: 0,
            weapon_speed: 999999.0,
            min_weapon_speed: 999999.0,
            is_scale_weapon_speed: false,
            weapon_recoil: 0.0,
            min_target_pitch: -std::f32::consts::PI,
            max_target_pitch: std::f32::consts::PI,
            radius_damage_angle: std::f32::consts::PI,
            projectile_name: String::new(),
            projectile_stream_name: String::new(),
            laser_name: String::new(),
            laser_bone_name: String::new(),
            damage_type: DamageType::Explosion,
            damage_status_type: ObjectStatusTypes::new(ObjectStatusTypes::NONE),
            death_type: DeathType::Normal,
            anti_mask: WeaponAntiMask::new(WeaponAntiMask::GROUND),
            affects_mask: WeaponAffectsMask::new(
                WeaponAffectsMask::ALLIES
                    | WeaponAffectsMask::ENEMIES
                    | WeaponAffectsMask::NEUTRALS,
            ),
            collide_mask: WeaponCollideMask::new(WeaponCollideMask::STRUCTURES),
            damage_dealt_at_self_position: false,
            reload_type: WeaponReloadType::AutoReload,
            prefire_type: WeaponPrefireType::PrefirePerShot,
            leech_range_weapon: false,
            capable_of_following_waypoint: false,
            is_shows_ammo_pips: false,
            allow_attack_garrisoned_bldgs: false,
            play_fx_when_stealthed: false,
            die_on_detonate: false,
            must_travel_pfx: false,
            continuous_fire_one_shots_needed: i32::MAX,
            continuous_fire_two_shots_needed: i32::MAX,
            continuous_fire_coast_frames: 0,
            continue_attack_range: 0.0,
            infantry_inaccuracy_dist: 0.0,
            shots_per_barrel: 1,
            historic_bonus_time: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_count: 0,
            historic_bonus_weapon: None,
            historic_bonus_weapon_name: String::new(),
            fire_sound: AudioEventRts::new(String::new()),
            fire_sound_loop_time: 0,
            fire_fx: [None, None, None, None],
            projectile_detonate_fx: [None, None, None, None],
            fire_ocl: [None, None, None, None],
            projectile_detonation_ocl: [None, None, None, None],
            fire_ocl_names: [None, None, None, None],
            projectile_detonation_ocl_names: [None, None, None, None],
            projectile_exhaust: [None, None, None, None],
            projectile_exhaust_names: [None, None, None, None],
            extra_bonus: None,
            historic_damage: Arc::new(Mutex::new(VecDeque::new())),
            next_template: None,
        }
    }

    /// Get attack range with bonus applied
    pub fn get_attack_range(&self, bonus: &WeaponBonus) -> f32 {
        // Note: undersize by 1/4 of a pathfind cell to avoid edge cases
        const PATHFIND_CELL_SIZE: f32 = 10.0; // Assumed value
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        let range = self.attack_range * bonus.get_field(WeaponBonusField::Range) - UNDERSIZE;
        range.max(0.0)
    }

    /// Get unmodified attack range
    pub fn get_unmodified_attack_range(&self) -> f32 {
        self.attack_range
    }

    pub fn get_request_assist_range(&self) -> f32 {
        self.request_assist_range
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_min_target_pitch(&self) -> f32 {
        self.min_target_pitch
    }

    pub fn get_max_target_pitch(&self) -> f32 {
        self.max_target_pitch
    }

    pub fn get_shots_per_barrel(&self) -> i32 {
        self.shots_per_barrel
    }

    pub fn get_clip_size(&self) -> i32 {
        self.clip_size
    }

    pub fn get_scatter_targets_count(&self) -> usize {
        self.scatter_targets.len()
    }

    pub fn get_scatter_targets_vector(&self) -> &[Coord2D] {
        &self.scatter_targets
    }

    pub fn get_scatter_target_scalar(&self) -> f32 {
        self.scatter_target_scalar
    }

    pub fn is_leech_range_weapon(&self) -> bool {
        self.leech_range_weapon
    }

    pub fn get_anti_mask(&self) -> u32 {
        self.anti_mask.bits()
    }

    pub fn get_extra_bonus(&self) -> Option<&WeaponBonusSet> {
        self.extra_bonus.as_ref()
    }

    /// Get minimum attack range
    pub fn get_minimum_attack_range(&self) -> f32 {
        const PATHFIND_CELL_SIZE: f32 = 10.0; // Assumed value
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        let range = self.minimum_attack_range - UNDERSIZE;
        range.max(0.0)
    }

    /// Get delay between shots with bonus applied
    /// C++ Weapon.cpp line 475: WeaponTemplate::getDelayBetweenShots
    /// Uses GameLogicRandomValue for replay-deterministic randomization.
    pub fn get_delay_between_shots(&self, bonus: &WeaponBonus) -> i32 {
        // C++ Weapon.cpp line 480-483: Random number thing doesn't like min==max case
        let delay = if self.min_delay_between_shots == self.max_delay_between_shots {
            self.min_delay_between_shots
        } else {
            get_game_logic_random_value(self.min_delay_between_shots, self.max_delay_between_shots)
        };

        let bonus_rof = bonus.get_field(WeaponBonusField::RateOfFire);
        // C++ Weapon.cpp line 489: REAL_TO_INT_FLOOR(delayToUse / bonusROF)
        ((delay as f32) / bonus_rof).floor() as i32
    }

    /// Get clip reload time with bonus applied
    pub fn get_clip_reload_time(&self, bonus: &WeaponBonus) -> i32 {
        let bonus_rof = bonus.get_field(WeaponBonusField::RateOfFire);
        ((self.clip_reload_time as f32) / bonus_rof).floor() as i32
    }

    /// Get pre-attack delay with bonus applied
    pub fn get_pre_attack_delay(&self, bonus: &WeaponBonus) -> i32 {
        ((self.pre_attack_delay as f32) * bonus.get_field(WeaponBonusField::PreAttack)) as i32
    }

    /// Get primary damage with bonus applied
    pub fn get_primary_damage(&self, bonus: &WeaponBonus) -> f32 {
        self.primary_damage * bonus.get_field(WeaponBonusField::Damage)
    }

    /// Get primary damage radius with bonus applied
    pub fn get_primary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        self.primary_damage_radius * bonus.get_field(WeaponBonusField::Radius)
    }

    /// Get secondary damage with bonus applied
    pub fn get_secondary_damage(&self, bonus: &WeaponBonus) -> f32 {
        self.secondary_damage * bonus.get_field(WeaponBonusField::Damage)
    }

    /// Get secondary damage radius with bonus applied
    pub fn get_secondary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        self.secondary_damage_radius * bonus.get_field(WeaponBonusField::Radius)
    }

    /// Check if this is a contact weapon (requires collision with target)
    ///
    /// Matches C++ WeaponTemplate::isContactWeapon() from Weapon.cpp lines 531-543
    /// A weapon is a contact weapon if its attack range (minus undersize) is less than
    /// one pathfind cell size. This ensures weapons that require close proximity
    /// (melee, collision-based) are correctly identified.
    pub fn is_contact_weapon(&self) -> bool {
        // Note: undersize by 1/4 of a pathfind cell to avoid edge cases with
        // goal positions teetering on the edge of firing range
        const PATHFIND_CELL_SIZE: f32 = 10.0;
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        // Contact weapon if attack range after undersize is less than one cell
        (self.attack_range - UNDERSIZE) < PATHFIND_CELL_SIZE
    }

    /// Check if this weapon automatically reloads
    pub fn get_auto_reloads_clip(&self) -> bool {
        matches!(self.reload_type, WeaponReloadType::AutoReload)
    }

    /// Check if this is a laser weapon
    pub fn is_laser(&self) -> bool {
        !self.laser_name.is_empty()
    }

    /// Set the next template for inheritance
    pub fn set_next_template(&mut self, next_template: WeaponTemplate) {
        self.next_template = Some(Box::new(next_template));
    }

    /// Check if this template is an override
    pub fn is_override(&self) -> bool {
        self.next_template.is_some()
    }

    /// Fire the weapon template with full ballistics calculation
    pub fn fire_weapon_template(
        &self,
        source_obj: ObjectId,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
        victim_obj: Option<ObjectId>,
        victim_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        firing_weapon: Option<&mut Weapon>,
        inflict_damage: bool,
    ) -> GameLogicResult<u32> {
        let source_pos = self.get_object_position(source_obj)?;
        let target_pos = match (victim_obj, victim_pos) {
            (Some(obj_id), _) => self.get_object_position(obj_id)?,
            (None, Some(pos)) => *pos,
            _ => {
                return Err(GameLogicError::Configuration(
                    "No valid target specified".to_string(),
                ));
            }
        };

        // 1. Validate target and range
        if !ignore_ranges && !self.is_target_in_range(&source_pos, &target_pos, bonus) {
            return Ok(0);
        }

        // 2. Apply scatter (C++ Weapon.cpp lines ~953-1008)
        let mut projectile_destination = target_pos;
        let mut launch_victim_obj = victim_obj;
        let mut scatter_radius = self.scatter_radius;
        let mut target_layer = PathfindLayerEnum::Ground;
        let mut victim_is_infantry = false;

        if let Some(victim_id) = victim_obj {
            if let Some(victim_arc) = TheGameLogic::find_object_by_id(victim_id) {
                if let Ok(victim_guard) = victim_arc.read() {
                    target_layer = victim_guard.get_layer();
                    if victim_guard.is_structure() {
                        projectile_destination = victim_guard
                            .get_geometry_info()
                            .get_center_position(victim_guard.get_position());
                    }
                    if self.infantry_inaccuracy_dist > 0.0
                        && victim_guard.is_kind_of(KindOf::Infantry)
                    {
                        victim_is_infantry = true;
                    }
                }
            }
        }

        if self.infantry_inaccuracy_dist > 0.0 && victim_is_infantry {
            scatter_radius += self.infantry_inaccuracy_dist;
        }

        if scatter_radius > 0.0 {
            let scatter_amount = get_game_logic_random_value_real(0.0, scatter_radius);
            let scatter_angle = get_game_logic_random_value_real(0.0, 2.0 * std::f32::consts::PI);
            projectile_destination.x += scatter_amount * scatter_angle.cos();
            projectile_destination.y += scatter_amount * scatter_angle.sin();
            if let Some(terrain) = TheTerrainLogic::get() {
                projectile_destination.z = terrain.get_layer_height(
                    projectile_destination.x,
                    projectile_destination.y,
                    target_layer,
                );
            }
            launch_victim_obj = None;
        }

        // 3. Calculate ballistics trajectory if this is a projectile weapon
        let damage_frame = if self.is_contact_weapon() {
            // Contact weapon - immediate damage
            self.calculate_contact_damage(
                source_obj,
                victim_obj,
                &target_pos,
                bonus,
                inflict_damage,
            )?
        } else {
            // Projectile weapon - calculate flight time (C++ uses simple distance/speed)
            let distance = source_pos.distance(projectile_destination);
            let effective_speed = self.weapon_speed * bonus.get_field(WeaponBonusField::Range);
            let flight_time = if effective_speed > 0.0 {
                distance / effective_speed
            } else {
                0.0
            };

            let flight_time_frames = (flight_time * LOGICFRAMES_PER_SECOND as f32) as u32;
            let current_frame = self.get_current_frame();

            // Create projectile if needed
            if !self.projectile_name.is_empty() {
                let projectile_id = self.create_projectile(
                    source_obj,
                    &source_pos,
                    &projectile_destination,
                    bonus,
                    launch_victim_obj,
                    weapon_slot,
                    specific_barrel_to_use,
                )?;
                if let Some(firing_weapon) = firing_weapon {
                    firing_weapon.new_projectile_fired(
                        source_obj,
                        projectile_id,
                        victim_obj,
                        Some(&target_pos),
                    );
                }

                let is_missile = self.projectile_has_behavior("MissileAIUpdate")
                    || self.projectile_has_behavior("SmartBombTargetHomingUpdate");
                if is_missile {
                    if let Some(victim_id) = victim_obj {
                        if let Some(victim_arc) = TheGameLogic::find_object_by_id(victim_id) {
                            if let Ok(mut victim_guard) = victim_arc.write() {
                                for behavior in victim_guard.get_behavior_modules() {
                                    let Ok(mut behavior) = behavior.lock() else {
                                        continue;
                                    };
                                    if let Some(countermeasures) =
                                        behavior.get_countermeasures_behavior_interface()
                                    {
                                        let _ = countermeasures
                                            .report_missile_for_countermeasures(projectile_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            current_frame + flight_time_frames
        };

        // 3. Apply immediate effects (muzzle flash, sound, etc.)
        self.apply_firing_effects(source_obj, &source_pos, weapon_slot)?;

        // 4. Handle scatter targets if configured
        if !self.scatter_targets.is_empty() {
            self.handle_scatter_targets(source_obj, &target_pos, bonus, inflict_damage)?;
        }

        Ok(damage_frame)
    }

    /// Get object position helper method
    fn get_object_position(&self, obj_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
            return Err(GameLogicError::InvalidObject(obj_id));
        };
        let obj_guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock object position".to_string()))?;
        Ok(*obj_guard.get_position())
    }

    /// Check if target is in range
    fn is_target_in_range(
        &self,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
    ) -> bool {
        let distance = source_pos.distance(*target_pos);
        let attack_range = self.get_attack_range(bonus);
        let min_range = self.get_minimum_attack_range();

        distance <= attack_range && distance >= min_range
    }

    /// Calculate contact weapon damage (immediate)
    fn calculate_contact_damage(
        &self,
        source_obj: ObjectId,
        victim_obj: Option<ObjectId>,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> GameLogicResult<u32> {
        if inflict_damage {
            // Apply damage immediately
            let primary_damage = self.get_primary_damage(bonus);
            let primary_radius = self.get_primary_damage_radius(bonus);

            // Damage calculation would happen here
            log::debug!(
                "Contact weapon damage: {} at radius {} from {:?}",
                primary_damage,
                primary_radius,
                target_pos
            );

            self.apply_historic_bonus(source_obj, target_pos);
        }

        Ok(self.get_current_frame()) // Return current frame for immediate damage
    }

    /// Create projectile
    fn create_projectile(
        &self,
        source_obj: ObjectId,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
        victim_obj: Option<ObjectId>,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
    ) -> GameLogicResult<ObjectId> {
        log::debug!(
            "Creating projectile '{}' from {:?} to {:?}",
            self.projectile_name,
            source_pos,
            target_pos
        );

        if let Some(projectile_template) = TheObjectFactory::find_template(&self.projectile_name) {
            let mut owning_player = None;
            let mut projectile_team = None;
            let mut source_veterancy = crate::common::VeterancyLevel::Regular;

            if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj) {
                if let Ok(source_guard) = source_arc.read() {
                    owning_player = source_guard.get_controlling_player();
                    source_veterancy = source_guard.get_veterancy_level();
                    if let Some(player_arc) = &owning_player {
                        if let Ok(player_guard) = player_arc.read() {
                            projectile_team = player_guard.get_default_team();
                        }
                    }
                    if projectile_team.is_none() {
                        projectile_team = source_guard.get_team();
                    }
                }
            }

            let projectile_arc = TheObjectFactory::new_object(
                projectile_template,
                projectile_team.as_ref().map(Arc::clone),
            )
            .map_err(|e| {
                GameLogicError::Configuration(format!("Projectile create failed: {}", e))
            })?;

            let projectile_id = projectile_arc
                .read()
                .map_err(|_| GameLogicError::Threading("Projectile lock poisoned".into()))?
                .get_id();

            {
                let mut proj_guard = projectile_arc
                    .write()
                    .map_err(|_| GameLogicError::Threading("Projectile lock poisoned".into()))?;
                let _ = proj_guard.set_position(source_pos);

                if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj) {
                    if let Ok(source_guard) = source_arc.read() {
                        proj_guard.set_producer(Some(&source_guard));
                        if source_guard.notify_special_power_completion_die() {
                            proj_guard.set_special_power_completion_creator(INVALID_OBJECT_ID);
                        } else {
                            proj_guard.set_special_power_completion_creator(source_obj);
                        }
                    }
                }
            }

            Self::position_projectile_for_launch(
                &projectile_arc,
                source_obj,
                weapon_slot,
                specific_barrel_to_use,
            )?;

            if let Some(player_arc) = owning_player {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.get_num_battle_plans_active() > 0 {
                        if let Ok(mut proj_guard) = projectile_arc.write() {
                            player_guard.apply_battle_plan_bonuses_for_object(&mut proj_guard);
                        }
                    }
                }
            }

            let exhaust = self.get_projectile_exhaust(source_veterancy);

            let weapon_template = Arc::new(self.clone());
            let mut launched = false;
            if let Ok(mut proj_guard) = projectile_arc.write() {
                let modules = proj_guard.behavior_modules();
                drop(proj_guard);

                for module in modules {
                    let mut did_launch = false;
                    module.with_module(|behavior| {
                        let Some(projectile_behavior) = module_projectile_launch_kind(behavior)
                        else {
                            return;
                        };

                        match projectile_behavior {
                            ProjectileLaunchKindMut::MissileAIUpdateBehavior(missile) => {
                                missile.projectile_launch_at_object_or_position(
                                    victim_obj,
                                    target_pos,
                                    Some(source_obj),
                                    weapon_slot,
                                    specific_barrel_to_use,
                                    Some(Arc::downgrade(&weapon_template)),
                                    exhaust.clone(),
                                );
                                did_launch = true;
                            }
                            ProjectileLaunchKindMut::NeutronMissileUpdate(neutron) => {
                                let exhaust_name = exhaust.as_ref().map(|tmpl| tmpl.name.clone());
                                if let Some(launcher_arc) =
                                    TheGameLogic::find_object_by_id(source_obj)
                                {
                                    if let Ok(launcher_guard) = launcher_arc.read() {
                                        if let Some(victim_id) = victim_obj {
                                            if let Some(victim_arc) =
                                                TheGameLogic::find_object_by_id(victim_id)
                                            {
                                                if let Ok(victim_guard) = victim_arc.read() {
                                                    neutron
                                                        .projectile_launch_at_object_or_position(
                                                            Some(&victim_guard),
                                                            Some(target_pos),
                                                            Some(&launcher_guard),
                                                            map_weapon_slot_to_common(weapon_slot),
                                                            specific_barrel_to_use,
                                                            Some(&weapon_template),
                                                            exhaust_name.clone(),
                                                        );
                                                    did_launch = true;
                                                }
                                            }
                                        } else {
                                            neutron.projectile_launch_at_object_or_position(
                                                None,
                                                Some(target_pos),
                                                Some(&launcher_guard),
                                                map_weapon_slot_to_common(weapon_slot),
                                                specific_barrel_to_use,
                                                Some(&weapon_template),
                                                exhaust_name,
                                            );
                                            did_launch = true;
                                        }
                                    }
                                }
                            }
                            ProjectileLaunchKindMut::DumbProjectileBehavior(dumb) => {
                                dumb.projectile_launch_at_object_or_position(
                                    victim_obj,
                                    target_pos,
                                    source_obj,
                                    weapon_slot,
                                    specific_barrel_to_use,
                                    Some(Arc::clone(&weapon_template)),
                                );
                                did_launch = true;
                            }
                        }
                    });

                    if did_launch {
                        launched = true;
                        break;
                    }
                }
            }

            if !launched {
                if let Ok(mut proj_guard) = projectile_arc.write() {
                    let _ = proj_guard.set_position(target_pos);
                }
            }

            return Ok(projectile_id);
        }

        Err(GameLogicError::Configuration(format!(
            "Projectile template '{}' not found",
            self.projectile_name
        )))
    }

    fn calc_projectile_launch_position(
        launcher: &crate::object::Object,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
    ) -> (Matrix3D, Coord3D) {
        if let Some(container_id) = launcher.get_contained_by() {
            if let Some(container_arc) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container_arc.read() {
                    if let Some(contain_arc) = container_guard.get_contain() {
                        if let Ok(contain_guard) = contain_arc.lock() {
                            if contain_guard.is_enclosing_container_for(launcher) {
                                let world_transform = launcher.get_transform_matrix();
                                let (_, _, translation) =
                                    world_transform.to_scale_rotation_translation();
                                return (world_transform, translation);
                            }
                        }
                    }
                }
            }
        }

        let (turret, turret_angle, turret_pitch) =
            if let Some(ai) = launcher.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_weapon_slot(weapon_slot);
                    let (angle, pitch) = ai_guard
                        .get_turret_rot_and_pitch(turret)
                        .unwrap_or((0.0, 0.0));
                    (turret, angle, pitch)
                } else {
                    (TurretType::Invalid, 0.0, 0.0)
                }
            } else {
                (TurretType::Invalid, 0.0, 0.0)
            };

        let mut attach_transform = Matrix3D::IDENTITY;
        let mut turret_rot_pos = Coord3D::ZERO;
        let mut turret_pitch_pos = Coord3D::ZERO;
        let mut found_launch_offset = false;
        if let Some(drawable) = launcher.get_drawable() {
            if let Some(launch) = drawable.get_projectile_launch_offset(
                map_weapon_slot_to_common(weapon_slot),
                specific_barrel_to_use,
                turret,
            ) {
                attach_transform = launch.transform;
                turret_rot_pos = launch.turret_rot_pos;
                turret_pitch_pos = launch.turret_pitch_pos;
                found_launch_offset = true;
            }
        }

        if !found_launch_offset {
            log::warn!(
                "ProjectileLaunchPos {:?} {} not found for launcher {}",
                weapon_slot,
                specific_barrel_to_use,
                launcher.get_id()
            );
            debug_assert!(
                false,
                "ProjectileLaunchPos {:?} {} not found for launcher {}",
                weapon_slot,
                specific_barrel_to_use,
                launcher.get_id()
            );
        }

        if turret != TurretType::Invalid {
            let pitch_adjustment = Matrix3D::from_translation(turret_pitch_pos)
                * Matrix3D::from_rotation_y(-turret_pitch)
                * Matrix3D::from_translation(-turret_pitch_pos);

            let turn_adjustment = Matrix3D::from_translation(turret_rot_pos)
                * Matrix3D::from_rotation_z(turret_angle)
                * Matrix3D::from_translation(-turret_rot_pos);

            attach_transform = turn_adjustment * pitch_adjustment * attach_transform;
        }

        let world_transform = launcher.convert_bone_pos_to_world_pos(None, Some(&attach_transform));
        let (_, _, translation) = world_transform.to_scale_rotation_translation();
        (world_transform, translation)
    }

    pub(crate) fn position_projectile_for_launch(
        projectile_arc: &Arc<RwLock<crate::object::Object>>,
        launcher_id: ObjectId,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
    ) -> GameLogicResult<()> {
        let Some(launcher_arc) = TheGameLogic::find_object_by_id(launcher_id) else {
            if let Ok(projectile_guard) = projectile_arc.read() {
                let _ = TheGameLogic::destroy_object_by_id(projectile_guard.get_id());
            }
            return Err(GameLogicError::InvalidObject(launcher_id));
        };

        let launcher_guard = launcher_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Launcher lock poisoned".into()))?;

        let (world_transform, world_pos) = Self::calc_projectile_launch_position(
            &launcher_guard,
            weapon_slot,
            specific_barrel_to_use,
        );

        let mut projectile_guard = projectile_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Projectile lock poisoned".into()))?;

        if let Some(drawable) = projectile_guard.get_drawable() {
            drawable.set_drawable_hidden(false);
        }

        projectile_guard.set_transform_matrix(&world_transform);
        let _ = projectile_guard.set_position(&world_pos);

        if let Some(tracker) = projectile_guard.get_experience_tracker() {
            if let Ok(mut tracker_guard) = tracker.lock() {
                tracker_guard.set_experience_sink(launcher_guard.get_id());
            }
        }

        let launcher_phys = launcher_guard.get_physics();
        let projectile_phys = projectile_guard.get_physics();
        drop(launcher_guard);

        if let (Some(launcher_phys), Some(projectile_phys)) = (launcher_phys, projectile_phys) {
            if let Ok(launcher_guard) = launcher_phys.lock() {
                let velocity = launcher_guard.get_velocity();
                if let Ok(mut projectile_guard) = projectile_phys.lock() {
                    projectile_guard.set_velocity(&velocity);
                    projectile_guard.set_ignore_collisions_with(launcher_id);
                }
            }
        }

        Ok(())
    }

    /// Apply immediate firing effects
    fn apply_firing_effects(
        &self,
        source_obj: ObjectId,
        source_pos: &Coord3D,
        weapon_slot: WeaponSlotType,
    ) -> GameLogicResult<()> {
        // Apply muzzle flash, sound effects, etc.
        if !self.fire_sound.is_empty() {
            log::debug!("Playing fire sound for weapon '{}'", self.name);
        }

        // Visual effects would be triggered here
        Ok(())
    }

    /// Handle scatter targets
    fn handle_scatter_targets(
        &self,
        source_obj: ObjectId,
        primary_target_pos: &Coord3D,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> GameLogicResult<()> {
        for scatter_target in &self.scatter_targets {
            let scatter_pos = Coord3D::new(
                primary_target_pos.x + scatter_target.x,
                primary_target_pos.y + scatter_target.y,
                primary_target_pos.z,
            );

            // Fire at scatter position
            self.calculate_contact_damage(source_obj, None, &scatter_pos, bonus, inflict_damage)?;
        }

        Ok(())
    }

    /// Get current game frame
    fn get_current_frame(&self) -> u32 {
        TheGameLogic::get_frame()
    }

    /// C++ Weapon.cpp:1169-1186 trimOldHistoricDamage — global historicDamageLimit.
    pub fn trim_old_historic_damage(&self) {
        let limit = game_engine::common::global_data::read().historic_damage_limit;
        let expiration = TheGameLogic::get_frame().saturating_sub(limit);
        if let Ok(mut damage_list) = self.historic_damage.lock() {
            while let Some(front) = damage_list.front() {
                if front.frame <= expiration {
                    damage_list.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Record historic damage for bonus calculations (no trim; C++ records after check).
    fn record_historic_damage(&self, location: &Coord3D, frame: u32) {
        if let Ok(mut damage_list) = self.historic_damage.lock() {
            damage_list.push_back(HistoricWeaponDamageInfo::new(frame, *location));
        }
    }

    /// Hits still inside the historic window (tests / honesty).
    pub fn historic_damage_len(&self) -> usize {
        self.historic_damage
            .lock()
            .map(|list| list.len())
            .unwrap_or(0)
    }

    /// C++ `INI::parseWeaponTemplate` for HistoricBonusWeapon.
    pub fn set_historic_bonus_weapon_name(&mut self, name: &str) {
        let name = name.split_whitespace().next().unwrap_or(name).trim();
        if name.is_empty() || name.eq_ignore_ascii_case("None") {
            self.historic_bonus_weapon_name.clear();
            self.historic_bonus_weapon = None;
        } else {
            self.historic_bonus_weapon_name = name.to_string();
        }
    }

    /// Fill HistoricBonusWeapon name from Common Weapon.ini when the Weak is empty.
    pub fn fill_historic_bonus_weapon_name(&mut self) {
        if !self.historic_bonus_weapon_name.trim().is_empty() {
            return;
        }
        if let Some(name) = common_historic_bonus_weapon_name(&self.name) {
            self.set_historic_bonus_weapon_name(&name);
        }
    }

    fn historic_bonus_weapon_name_resolved(&self) -> Option<String> {
        let named = self.historic_bonus_weapon_name.trim();
        if !named.is_empty() && !named.eq_ignore_ascii_case("None") {
            return Some(named.to_string());
        }
        if let Some(arc) = self
            .historic_bonus_weapon
            .as_ref()
            .and_then(|weak| weak.upgrade())
        {
            if !arc.name.is_empty() && !arc.name.eq_ignore_ascii_case(&self.name) {
                return Some(arc.name.clone());
            }
        }
        common_historic_bonus_weapon_name(&self.name)
    }

    /// Resolve HistoricBonusWeapon: live Weak, stored name, store lookup, Common INI.
    pub fn resolve_historic_bonus_weapon(&self) -> Option<Arc<WeaponTemplate>> {
        if let Some(arc) = self
            .historic_bonus_weapon
            .as_ref()
            .and_then(|weak| weak.upgrade())
        {
            if !arc.name.eq_ignore_ascii_case(&self.name) {
                return Some(arc);
            }
            return None;
        }
        let name = self.historic_bonus_weapon_name_resolved()?;
        if name.eq_ignore_ascii_case(&self.name) || name.eq_ignore_ascii_case("None") {
            return None;
        }
        with_weapon_store(|store| store.find_weapon_template_ci(&name).cloned())
            .ok()
            .flatten()
    }

    /// C++ Weapon.cpp:1214-1251 — fire historic bonus then clear, else record this hit.
    ///
    /// Returns true when the Nth qualifying hit dispatched HistoricBonusWeapon
    /// via `WeaponStore::createAndFireTempWeapon`.
    pub fn apply_historic_bonus(&self, source_obj: ObjectId, pos: &Coord3D) -> bool {
        self.trim_old_historic_damage();
        // C++: if (m_historicBonusCount > 0 && m_historicBonusWeapon != this)
        if self.historic_bonus_count <= 0 {
            return false;
        }
        let bonus_weapon = self.resolve_historic_bonus_weapon();
        if bonus_weapon
            .as_ref()
            .is_some_and(|bonus| bonus.name.eq_ignore_ascii_case(&self.name))
        {
            return false;
        }

        let current_frame = TheGameLogic::get_frame();
        let rad_sqr = self.historic_bonus_radius * self.historic_bonus_radius;
        let oldest = current_frame.saturating_sub(self.historic_bonus_time);
        let count = if let Ok(list) = self.historic_damage.lock() {
            list.iter()
                .filter(|info| {
                    // C++: it->frame >= oldestThatWillCount && 2D dist²
                    if info.frame < oldest {
                        return false;
                    }
                    let dx = info.location.x - pos.x;
                    let dy = info.location.y - pos.y;
                    dx * dx + dy * dy <= rad_sqr
                })
                .count() as i32
        } else {
            0
        };

        if count >= self.historic_bonus_count - 1 {
            // minus 1: this hit is included implicitly (Weapon.cpp:1233)
            let dispatched = if let Some(bonus_weapon) = bonus_weapon {
                let _ = with_weapon_store(|store| {
                    store.create_and_fire_temp_weapon(&bonus_weapon, source_obj, None, Some(pos))
                });
                true
            } else {
                false
            };
            if let Ok(mut list) = self.historic_damage.lock() {
                list.clear();
            }
            dispatched
        } else {
            self.record_historic_damage(pos, current_frame);
            false
        }
    }

    /// C++ Weapon.cpp estimateWeaponTemplateDamage: returns estimated damage to
    /// a victim, taking bonuses and armor into account. Does NOT consider range.
    pub fn estimate_weapon_template_damage(
        &self,
        source_obj: ObjectID,
        victim_obj: Option<ObjectID>,
        victim_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
    ) -> f32 {
        let _ = victim_pos; // C++ ignores victim position once victim object is known.
        let primary_damage = self.get_primary_damage(bonus);
        let Some(victim_id) = victim_obj else {
            return primary_damage;
        };

        let source_id = if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj) {
            if let Ok(source_guard) = source_arc.read() {
                source_guard.get_id()
            } else {
                source_obj
            }
        } else {
            source_obj
        };

        let Some(victim_arc) = TheGameLogic::find_object_by_id(victim_id) else {
            return primary_damage;
        };
        let Ok(victim_guard) = victim_arc.read() else {
            return primary_damage;
        };

        if let Some(special) = estimate_weapon_targetability_specials(
            self.damage_type,
            self.death_type,
            self.allow_attack_garrisoned_bldgs,
            &*victim_guard,
        ) {
            return special;
        }

        let damage_info = crate::damage::DamageInfoInput {
            damage_type: crate::damage::DamageType::from_u32(self.damage_type as u32),
            death_type: crate::damage::DeathType::from_u32(self.death_type as u32),
            source_id,
            amount: primary_damage,
            ..Default::default()
        };
        victim_guard.estimate_damage(&damage_info)
    }

    pub fn get_projectile_speed(&self) -> crate::common::Real {
        self.weapon_speed.max(self.min_weapon_speed)
    }

    pub fn get_projectile_lifetime(&self) -> crate::common::Real {
        if let Some(lifetime) = self.projectile_missile_fuel_lifetime_seconds() {
            return lifetime;
        }
        if self.continuous_fire_coast_frames > 0 {
            return self.continuous_fire_coast_frames as crate::common::Real
                / LOGICFRAMES_PER_SECOND as crate::common::Real;
        }
        crate::common::Real::INFINITY
    }

    pub fn get_initial_velocity(&self) -> crate::common::Real {
        self.projectile_missile_initial_velocity()
            .unwrap_or_else(|| self.weapon_speed.max(self.min_weapon_speed))
    }

    pub fn get_damage_type(&self) -> DamageType {
        self.damage_type
    }

    pub fn get_fire_fx(&self, level: crate::common::VeterancyLevel) -> Option<&FXList> {
        self.fire_fx.get(level as usize).and_then(|fx| fx.as_ref())
    }

    pub fn get_projectile_detonate_fx(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<&FXList> {
        self.projectile_detonate_fx
            .get(level as usize)
            .and_then(|fx| fx.as_ref())
    }

    /// Resolve the C++ `FireOCL` reference for this veterancy level.
    ///
    /// Weapon.ini is loaded before or independently from ObjectCreationList.ini
    /// in several Rust startup paths. Keeping the name and resolving it at use
    /// time matches C++ post-load resolution without manufacturing an empty OCL
    /// for a missing asset.
    pub fn get_fire_ocl(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<Arc<ObjectCreationList>> {
        let index = level as usize;
        self.fire_ocl.get(index).cloned().flatten().or_else(|| {
            self.fire_ocl_names
                .get(index)
                .and_then(|name| name.as_deref())
                .and_then(TheObjectCreationListStore::find_object_creation_list)
        })
    }

    /// Resolve the C++ `ProjectileDetonationOCL` reference for this veterancy
    /// level without inventing an OCL when its definition is absent.
    pub fn get_projectile_detonation_ocl(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<Arc<ObjectCreationList>> {
        let index = level as usize;
        self.projectile_detonation_ocl
            .get(index)
            .cloned()
            .flatten()
            .or_else(|| {
                self.projectile_detonation_ocl_names
                    .get(index)
                    .and_then(|name| name.as_deref())
                    .and_then(TheObjectCreationListStore::find_object_creation_list)
            })
    }

    /// Raw `FireOCL` name, retained for host-side presentation paths that use
    /// names rather than direct ObjectCreationList handles.
    pub fn get_fire_ocl_name(&self, level: crate::common::VeterancyLevel) -> Option<&str> {
        self.fire_ocl_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }

    /// Raw `ProjectileDetonationOCL` name, retained until it can be resolved
    /// against the live ObjectCreationList store.
    pub fn get_projectile_detonation_ocl_name(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<&str> {
        self.projectile_detonation_ocl_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }

    /// Resolve the C++ `ProjectileExhaust` particle template for this
    /// veterancy level. A missing particle definition remains absent: we keep
    /// its parsed name but never synthesize a generic particle template.
    pub fn get_projectile_exhaust(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<Arc<ParticleSystemTemplate>> {
        let index = level as usize;
        self.projectile_exhaust
            .get(index)
            .cloned()
            .flatten()
            .or_else(|| {
                self.projectile_exhaust_names
                    .get(index)
                    .and_then(|name| name.as_deref())
                    .and_then(|name| {
                        IniParticleSys::find_template_by_name(&AsciiString::from(name))
                    })
                    .map(Arc::new)
            })
    }

    /// Raw `ProjectileExhaust` name. This is intentionally separate from
    /// `get_projectile_exhaust`: presentation can wait for the client template
    /// loader, while simulation stays fail-closed when the definition is gone.
    pub fn get_projectile_exhaust_name(
        &self,
        level: crate::common::VeterancyLevel,
    ) -> Option<&str> {
        self.projectile_exhaust_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }

    pub fn get_homing_delay(&self) -> crate::common::Real {
        if self.is_guided() {
            return self.projectile_missile_homing_delay().unwrap_or(0.0);
        }
        0.0
    }

    pub fn get_homing_force(&self) -> crate::common::Real {
        if self.is_guided() { 1.0 } else { 0.0 }
    }

    pub fn is_guided(&self) -> bool {
        self.projectile_has_behavior("MissileAIUpdate")
            || self.projectile_has_behavior("SmartBombTargetHomingUpdate")
    }
    pub fn has_arc_trajectory(&self) -> bool {
        self.projectile_has_behavior("DumbProjectileBehavior")
    }
}

/// C++ WeaponTemplate::estimateWeaponTemplateDamage specials (Weapon.cpp:562-614).
fn estimate_weapon_targetability_specials(
    damage_type: DamageType,
    death_type: DeathType,
    allow_attack_garrisoned_bldgs: bool,
    victim: &crate::object::Object,
) -> Option<f32> {
    if victim.is_kind_of(KindOf::Shrubbery) {
        return Some(if death_type == DeathType::Burned {
            1.0
        } else {
            0.0
        });
    }
    if victim.is_kind_of(KindOf::Structure) && damage_type == DamageType::Sniper {
        if let Some(contain) = victim.get_contain() {
            if let Ok(guard) = contain.try_lock() {
                if guard.get_contained_count() == 0 {
                    return Some(0.0);
                }
            }
        }
    }
    if damage_type == DamageType::Surrender || allow_attack_garrisoned_bldgs {
        if let Some(contain) = victim.get_contain() {
            if let Ok(guard) = contain.try_lock() {
                if guard.get_contained_count() > 0
                    && guard.is_garrisonable()
                    && !guard.is_immune_to_clear_building_attacks()
                {
                    return Some(1.0);
                }
            }
        }
    }
    if damage_type == DamageType::Disarm {
        if victim.is_kind_of(KindOf::Mine)
            || victim.is_kind_of(KindOf::BoobyTrap)
            || victim.is_kind_of(KindOf::Demotrap)
        {
            return Some(1.0);
        }
        return Some(0.0);
    }
    if damage_type == DamageType::Deploy && !victim.is_airborne_target() {
        return Some(1.0);
    }
    None
}

/// Weapon.ini `HistoricBonusWeapon` is stored on the Common parser template
/// even when the GameLogic Weak has not been wired yet.
fn common_historic_bonus_weapon_name(owner: &str) -> Option<String> {
    game_engine::common::ini::ini_weapon::initialize_weapon_store();
    let store = game_engine::common::ini::ini_weapon::get_weapon_store()?;
    let tmpl = store.find_template(&AsciiString::from(owner))?;
    let raw = tmpl.properties.get("HistoricBonusWeapon")?;
    let name = raw.split_whitespace().next().unwrap_or(raw).trim();
    if name.is_empty() || name.eq_ignore_ascii_case("None") {
        None
    } else {
        Some(name.to_string())
    }
}
