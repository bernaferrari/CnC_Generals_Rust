//! Weapon Template System
//!
//! This module provides complete weapon template functionality matching the C++ implementation,
//! including all weapon properties, damage calculations, bonuses, and firing mechanics.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::KindOf;
use crate::common::{
    Coord2D, Coord3D, DamageType, DeathType, ObjectID, ObjectStatusTypes, PathfindLayerEnum,
    PlayerMaskType, Relationship, VeterancyLevel, LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfo, DamageInfoInput};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::TheThingFactory;
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModule;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::{
    projectile_launch_cast::{module_projectile_launch_kind, ProjectileLaunchKindMut},
    AudioEventRts, HistoricWeaponDamageInfo, WeaponAffectsMask, WeaponAntiMask, WeaponBonus,
    WeaponBonusConditionFlags, WeaponBonusField, WeaponBonusSet, WeaponCollideMask,
    WeaponPrefireType, WeaponReloadType, WeaponSlotType, INVALID_OBJECT_ID,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::thing::module::ModuleInterfaceType;

/// Maximum shots limit constant matching C++
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7fffffff;

fn map_weapon_slot_to_common(slot: WeaponSlotType) -> crate::common::WeaponSlotType {
    match slot {
        WeaponSlotType::Primary => crate::common::WeaponSlotType::Primary,
        WeaponSlotType::Secondary => crate::common::WeaponSlotType::Secondary,
        WeaponSlotType::Tertiary => crate::common::WeaponSlotType::Tertiary,
    }
}

/// Weapon template defining all weapon properties and behavior
#[derive(Debug, Clone)]
pub struct WeaponTemplate {
    /// Basic identification
    pub name: String,
    pub name_key: u32,

    /// Damage properties matching C++ exactly
    pub primary_damage: f32,
    pub primary_damage_radius: f32,
    pub secondary_damage: f32,
    pub secondary_damage_radius: f32,
    pub shock_wave_amount: f32,
    pub shock_wave_radius: f32,
    pub shock_wave_taper_off: f32,

    /// Range and targeting properties
    pub attack_range: f32,
    pub minimum_attack_range: f32,
    pub request_assist_range: f32,
    pub aim_delta: f32,
    pub scatter_radius: f32,
    pub scatter_target_scalar: f32,
    pub scatter_targets: Vec<Coord2D>,

    /// Timing and reload properties
    pub min_delay_between_shots: i32,
    pub max_delay_between_shots: i32,
    pub clip_size: i32,
    pub clip_reload_time: i32,
    pub pre_attack_delay: i32,
    pub auto_reload_when_idle_frames: u32,
    pub suspend_fx_delay: u32,

    /// Weapon behavior properties
    pub weapon_speed: f32,
    pub min_weapon_speed: f32,
    pub is_scale_weapon_speed: bool,
    pub weapon_recoil: f32,
    pub min_target_pitch: f32,
    pub max_target_pitch: f32,
    pub radius_damage_angle: f32,

    /// Projectile properties
    pub projectile_name: String,
    pub projectile_stream_name: String,
    pub laser_name: String,
    pub laser_bone_name: String,

    /// Damage and death types
    pub damage_type: DamageType,
    pub damage_status_type: ObjectStatusTypes,
    pub death_type: DeathType,

    /// Masks and flags for targeting and collision
    pub anti_mask: WeaponAntiMask,
    pub affects_mask: WeaponAffectsMask,
    pub collide_mask: WeaponCollideMask,

    /// Weapon type and behavior flags
    pub damage_dealt_at_self_position: bool,
    pub reload_type: WeaponReloadType,
    pub prefire_type: WeaponPrefireType,
    pub leech_range_weapon: bool,
    pub capable_of_following_waypoint: bool,
    pub is_shows_ammo_pips: bool,
    pub allow_attack_garrisoned_bldgs: bool,
    pub play_fx_when_stealthed: bool,
    pub die_on_detonate: bool,
    /// Whether projectile must use a trail particle effect
    pub must_travel_pfx: bool,

    /// Continuous fire properties
    pub continuous_fire_one_shots_needed: i32,
    pub continuous_fire_two_shots_needed: i32,
    pub continuous_fire_coast_frames: u32,

    /// Special targeting properties
    pub continue_attack_range: f32,
    pub infantry_inaccuracy_dist: f32,

    /// Barrel management
    pub shots_per_barrel: i32,

    /// Historic bonus system
    pub historic_bonus_time: u32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_count: i32,
    pub historic_bonus_weapon: Option<Weak<WeaponTemplate>>,

    /// Audio properties
    pub fire_sound: AudioEventRts,
    pub fire_sound_loop_time: u32,

    /// Per-veterancy level effects (Regular, Veteran, Elite, Heroic)
    pub fire_fx: [Option<FXList>; 4],
    pub projectile_detonate_fx: [Option<FXList>; 4],
    pub fire_ocl: [Option<ObjectCreationList>; 4],
    pub projectile_detonation_ocl: [Option<ObjectCreationList>; 4],
    pub projectile_exhaust: [Option<ParticleSystemTemplate>; 4],

    /// Bonus system
    pub extra_bonus: Option<WeaponBonusSet>,

    /// Historic damage tracking (thread-safe)
    historic_damage: Arc<Mutex<VecDeque<HistoricWeaponDamageInfo>>>,

    /// Template inheritance (for overrides)
    next_template: Option<Box<WeaponTemplate>>,
}

impl WeaponTemplate {
    /// Create a new weapon template with default values matching C++
    ///
    /// Matches C++ WeaponTemplate::WeaponTemplate() from Weapon.cpp lines 231-306
    pub fn new(name: String) -> Self {
        Self {
            name,
            name_key: 0, // NAMEKEY_INVALID
            primary_damage: 0.0,
            primary_damage_radius: 0.0,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0, // C++ line 248
            attack_range: 0.0,
            minimum_attack_range: 0.0,
            request_assist_range: 0.0,
            aim_delta: 0.0,
            scatter_radius: 0.0,
            scatter_target_scalar: 0.0, // C++ line 245
            scatter_targets: Vec::new(),
            min_delay_between_shots: 0,
            max_delay_between_shots: 0,
            clip_size: 0, // C++ line 276: m_clipSize = 0 (0 means unlimited)
            clip_reload_time: 0,
            pre_attack_delay: 0,
            auto_reload_when_idle_frames: 0,
            suspend_fx_delay: 0,
            weapon_speed: 999999.0,     // C++ line 251: effectively instant
            min_weapon_speed: 999999.0, // C++ line 252: effectively instant
            is_scale_weapon_speed: false,
            weapon_recoil: 0.0,
            min_target_pitch: -std::f32::consts::PI, // C++ line 255: -PI
            max_target_pitch: std::f32::consts::PI,  // C++ line 256: PI
            radius_damage_angle: std::f32::consts::PI, // C++ line 257: PI each way, full circle
            projectile_name: String::new(),
            projectile_stream_name: String::new(),
            laser_name: String::new(),
            laser_bone_name: String::new(),
            damage_type: DamageType::Explosion,
            damage_status_type: ObjectStatusTypes::None,
            death_type: DeathType::Normal,
            anti_mask: WeaponAntiMask::new(WeaponAntiMask::GROUND), // C++ line 284: WEAPON_ANTI_GROUND
            affects_mask: WeaponAffectsMask::new(0),
            collide_mask: WeaponCollideMask::new(0),
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
            continuous_fire_one_shots_needed: i32::MAX, // C++ line 282: INT_MAX
            continuous_fire_two_shots_needed: i32::MAX, // C++ line 283: INT_MAX
            continuous_fire_coast_frames: 0,
            continue_attack_range: 0.0,
            infantry_inaccuracy_dist: 0.0,
            shots_per_barrel: 1,
            historic_bonus_time: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_count: 0,
            historic_bonus_weapon: None,
            fire_sound: AudioEventRts::new(String::new()),
            fire_sound_loop_time: 0,
            fire_fx: [None, None, None, None],
            projectile_detonate_fx: [None, None, None, None],
            fire_ocl: [None, None, None, None],
            projectile_detonation_ocl: [None, None, None, None],
            projectile_exhaust: [None, None, None, None],
            extra_bonus: None,
            historic_damage: Arc::new(Mutex::new(VecDeque::new())),
            next_template: None,
        }
    }

    /// Get the weapon template name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn projectile_special_power_template(&self) -> Option<String> {
        let name = self.projectile_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("NONE") {
            return None;
        }
        let template = TheThingFactory::find_template(name)?;
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
        let name = self.projectile_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("NONE") {
            return false;
        }
        let Some(template) = TheThingFactory::find_template(name) else {
            return false;
        };
        template
            .get_behavior_module_info()
            .iter()
            .any(|info| info.name.as_str() == behavior_name)
    }

    fn target_is_infantry(victim_obj: Option<ObjectID>) -> bool {
        let Some(victim_id) = victim_obj else {
            return false;
        };
        let Some(victim_obj) = TheGameLogic::find_object_by_id(victim_id) else {
            return false;
        };
        let Ok(victim_guard) = victim_obj.read() else {
            return false;
        };
        victim_guard.is_kind_of(KindOf::Infantry)
    }

    fn effective_scatter_radius(&self, target_is_infantry: bool) -> f32 {
        if !target_is_infantry || self.infantry_inaccuracy_dist <= 0.0 {
            return self.scatter_radius;
        }
        self.scatter_radius + self.infantry_inaccuracy_dist
    }

    // ===== CORE WEAPON PROPERTIES WITH BONUS SUPPORT =====

    /// Get attack range with bonus applied (matches C++ exactly)
    pub fn get_attack_range(&self, bonus: &WeaponBonus) -> f32 {
        // Note: undersize by 1/4 of a pathfind cell to avoid edge cases
        const PATHFIND_CELL_SIZE: f32 = 10.0;
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        let range = self.attack_range * bonus.get_field(WeaponBonusField::Range) - UNDERSIZE;
        range.max(0.0)
    }

    /// Get unmodified attack range (C++ getUnmodifiedAttackRange)
    pub fn get_unmodified_attack_range(&self) -> f32 {
        self.attack_range
    }

    /// Get minimum attack range with undersize applied
    pub fn get_minimum_attack_range(&self) -> f32 {
        const PATHFIND_CELL_SIZE: f32 = 10.0;
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        let range = self.minimum_attack_range - UNDERSIZE;
        range.max(0.0)
    }

    /// Get delay between shots with bonus and randomization (matches C++)
    pub fn get_delay_between_shots(&self, bonus: &WeaponBonus) -> i32 {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let delay = if self.min_delay_between_shots == self.max_delay_between_shots {
            self.min_delay_between_shots
        } else {
            rng.gen_range(self.min_delay_between_shots..=self.max_delay_between_shots)
        };

        let bonus_rof = bonus.get_field(WeaponBonusField::RateOfFire);
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

    // ===== WEAPON TYPE IDENTIFICATION =====

    /// Check if this is a contact weapon (requires collision with target)
    ///
    /// Matches C++ WeaponTemplate::isContactWeapon() from Weapon.cpp lines 531-543
    pub fn is_contact_weapon(&self) -> bool {
        // Note: undersize by 1/4 of a pathfind cell to avoid edge cases
        const PATHFIND_CELL_SIZE: f32 = 10.0;
        const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;

        // Contact weapon if attack range after undersize is less than one cell
        (self.attack_range - UNDERSIZE) < PATHFIND_CELL_SIZE
    }

    /// Check if this weapon automatically reloads (matches C++ getAutoReloadsClip)
    pub fn get_auto_reloads_clip(&self) -> bool {
        matches!(self.reload_type, WeaponReloadType::AutoReload)
    }

    /// Check if this is a laser weapon
    pub fn is_laser(&self) -> bool {
        !self.laser_name.is_empty()
    }

    /// Check if this is a leech range weapon
    pub fn is_leech_range_weapon(&self) -> bool {
        self.leech_range_weapon
    }

    /// Get scatter targets vector
    pub fn get_scatter_targets_vector(&self) -> &[Coord2D] {
        &self.scatter_targets
    }

    /// Get scatter target scalar
    pub fn get_scatter_target_scalar(&self) -> f32 {
        self.scatter_target_scalar
    }

    /// Get scatter targets count
    pub fn get_scatter_targets_count(&self) -> usize {
        self.scatter_targets.len()
    }

    /// Get damage type
    pub fn get_damage_type(&self) -> DamageType {
        self.damage_type
    }

    /// Get anti-mask (what this weapon can target)
    pub fn get_anti_mask(&self) -> u32 {
        self.anti_mask.bits()
    }

    /// Get weapon speed
    pub fn get_weapon_speed(&self) -> f32 {
        self.weapon_speed
    }

    /// Get weapon recoil amount
    pub fn get_weapon_recoil_amount(&self) -> f32 {
        self.weapon_recoil
    }

    /// Get minimum target pitch
    pub fn get_min_target_pitch(&self) -> f32 {
        self.min_target_pitch
    }

    /// Get maximum target pitch
    pub fn get_max_target_pitch(&self) -> f32 {
        self.max_target_pitch
    }

    /// Get clip size
    pub fn get_clip_size(&self) -> i32 {
        self.clip_size
    }

    /// Get shots per barrel
    pub fn get_shots_per_barrel(&self) -> i32 {
        self.shots_per_barrel
    }

    /// Check if damage is dealt at self position
    pub fn get_damage_dealt_at_self_position(&self) -> bool {
        self.damage_dealt_at_self_position
    }

    /// Check if FX should play when stealthed
    pub fn is_play_fx_when_stealthed(&self) -> bool {
        self.play_fx_when_stealthed
    }

    // ===== VETERANCY-BASED EFFECTS ACCESS =====

    /// Get fire FX for veterancy level
    pub fn get_fire_fx(&self, veterancy: VeterancyLevel) -> Option<&FXList> {
        self.fire_fx.get(veterancy as usize)?.as_ref()
    }

    /// Get projectile detonate FX for veterancy level
    pub fn get_projectile_detonate_fx(&self, veterancy: VeterancyLevel) -> Option<&FXList> {
        self.projectile_detonate_fx
            .get(veterancy as usize)?
            .as_ref()
    }

    /// Get fire OCL for veterancy level
    pub fn get_fire_ocl(&self, veterancy: VeterancyLevel) -> Option<&ObjectCreationList> {
        self.fire_ocl.get(veterancy as usize)?.as_ref()
    }

    /// Get projectile detonation OCL for veterancy level
    pub fn get_projectile_detonation_ocl(
        &self,
        veterancy: VeterancyLevel,
    ) -> Option<&ObjectCreationList> {
        self.projectile_detonation_ocl
            .get(veterancy as usize)?
            .as_ref()
    }

    /// Get projectile exhaust for veterancy level
    pub fn get_projectile_exhaust(
        &self,
        veterancy: VeterancyLevel,
    ) -> Option<&ParticleSystemTemplate> {
        self.projectile_exhaust.get(veterancy as usize)?.as_ref()
    }

    // ===== TEMPLATE INHERITANCE SYSTEM =====

    /// Set the next template for inheritance (override system)
    pub fn set_next_template(&mut self, next_template: WeaponTemplate) {
        self.next_template = Some(Box::new(next_template));
    }

    /// Check if this template is an override
    pub fn is_override(&self) -> bool {
        self.next_template.is_some()
    }

    /// Get the next template in the inheritance chain
    pub fn get_next_template(&self) -> Option<&WeaponTemplate> {
        self.next_template.as_ref().map(|t| t.as_ref())
    }

    /// Get extra bonus set for this weapon
    ///
    /// Matches C++ WeaponTemplate::getExtraBonus() from Weapon.cpp line 1814
    pub fn get_extra_bonus(&self) -> Option<&WeaponBonusSet> {
        self.extra_bonus.as_ref()
    }

    // ===== PROJECTILE COLLISION SYSTEM =====

    /// Should projectile collide with target (matches C++ logic exactly)
    pub fn should_projectile_collide_with(
        &self,
        projectile_launcher: ObjectID,
        projectile: ObjectID,
        thing_we_collided_with: ObjectID,
        intended_victim_id: ObjectID, // Could be INVALID_OBJECT_ID for position shots
    ) -> bool {
        let Some(projectile_obj) = crate::helpers::TheGameLogic::find_object_by_id(projectile)
        else {
            return false;
        };
        let Some(collided_obj) =
            crate::helpers::TheGameLogic::find_object_by_id(thing_we_collided_with)
        else {
            return false;
        };

        let Ok(projectile_guard) = projectile_obj.read() else {
            return false;
        };
        let Ok(collided_guard) = collided_obj.read() else {
            return false;
        };

        // Always collide with intended target.
        if collided_guard.get_id() == intended_victim_id {
            return true;
        }

        if let Some(launcher_obj) =
            crate::helpers::TheGameLogic::find_object_by_id(projectile_launcher)
        {
            if let Ok(launcher_guard) = launcher_obj.read() {
                // Never hit your own launcher.
                if launcher_guard.get_id() == collided_guard.get_id() {
                    return false;
                }

                // If our launcher is inside the collided object, ignore collision.
                if launcher_guard.get_contained_by() == Some(collided_guard.get_id()) {
                    return false;
                }
            }
        }

        // Never bother burning already-burned things.
        if matches!(
            self.get_damage_type(),
            DamageType::Flame | DamageType::ParticleBeam
        ) && collided_guard.test_status(ObjectStatusTypes::Burned)
        {
            return false;
        }

        // Special case: projectiles targeting parked planes should not detonate on the airfield.
        if collided_guard.is_kind_of(KindOf::FSAirfield)
            && intended_victim_id != INVALID_OBJECT_ID
            && collided_guard
                .with_parking_place_behavior(|parking| {
                    parking.has_reserved_space(intended_victim_id)
                })
                .unwrap_or(false)
        {
            return false;
        }

        // If target has a sneaky targeting offset, do not collide this frame.
        if let Some(ai) = collided_guard.get_ai() {
            if let Ok(ai_guard) = ai.lock() {
                let mut offset = Coord3D::new(0.0, 0.0, 0.0);
                if ai_guard.get_sneaky_targeting_offset(&mut offset) {
                    return false;
                }
            }
        }

        let mut required_mask = 0u32;
        match projectile_guard.relationship_to(&collided_guard) {
            Relationship::Allies | Relationship::Ally => {
                required_mask |= WeaponCollideMask::ALLIES;
            }
            Relationship::Enemy => {
                required_mask |= WeaponCollideMask::ENEMIES;
            }
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

        // Missile kind-of markers are still being wired globally; use name heuristics for now.
        let collided_name = collided_guard.get_template_name().to_ascii_lowercase();
        if collided_name.contains("small_missile") || collided_name.contains("smallmissile") {
            required_mask |= WeaponCollideMask::SMALL_MISSILES;
        }
        if collided_name.contains("ballistic_missile") || collided_name.contains("ballistic") {
            required_mask |= WeaponCollideMask::BALLISTIC_MISSILES;
        }

        for flag in [
            WeaponCollideMask::ALLIES,
            WeaponCollideMask::ENEMIES,
            WeaponCollideMask::STRUCTURES,
            WeaponCollideMask::SHRUBBERY,
            WeaponCollideMask::PROJECTILE,
            WeaponCollideMask::WALLS,
            WeaponCollideMask::SMALL_MISSILES,
            WeaponCollideMask::BALLISTIC_MISSILES,
            WeaponCollideMask::CONTROLLED_STRUCTURES,
        ] {
            if (required_mask & flag) != 0 && self.collide_mask.contains(flag) {
                return true;
            }
        }

        false
    }

    // ===== CORE WEAPON FIRING SYSTEM =====

    /// Fire the weapon template with full damage calculation
    ///
    /// Matches C++ WeaponTemplate::fireWeaponTemplate() from Weapon.cpp line 738
    ///
    /// Returns the frame when damage will occur (current frame for immediate, future frame for delayed)
    #[allow(clippy::too_many_arguments)]
    pub fn fire_weapon_template(
        &self,
        source_obj: ObjectID,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
        victim_obj: Option<ObjectID>,
        victim_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        firing_weapon: Option<&mut crate::weapon::Weapon>,
        projectile_id: &mut Option<ObjectID>,
        inflict_damage: bool,
    ) -> GameLogicResult<u32> {
        *projectile_id = None;

        // C++ line 775: Validate source and target
        if victim_obj.is_none() && victim_pos.is_none() {
            return Ok(0);
        }

        let source_id = source_obj;
        let source_pos = self.get_object_position(source_obj)?;

        // Determine actual target position and ID (C++ lines 792-837)
        let mut actual_victim_obj = victim_obj;
        let mut actual_victim_pos = match victim_pos {
            Some(pos) => *pos,
            None => {
                if let Some(vid) = victim_obj {
                    self.get_object_position(vid)?
                } else {
                    return Ok(0);
                }
            }
        };

        // Bridge attack point selection (C++ lines 819-831)
        // Bridges have two targetable points at either end - select the closer one
        if let Some(victim_id) = actual_victim_obj {
            if let Some(victim_obj_arc) = TheGameLogic::find_object_by_id(victim_id) {
                if let Ok(victim_guard) = victim_obj_arc.read() {
                    if victim_guard.is_kind_of(KindOf::Bridge) {
                        // Get bridge attack points
                        let mut info = crate::terrain::BridgeAttackInfo::new();
                        if let Some(_terrain) = TheTerrainLogic::get() {
                            let terrain_logic = crate::terrain::get_terrain_logic();
                            if let Ok(terrain_guard) = terrain_logic.read() {
                                terrain_guard.get_bridge_attack_points(victim_id, &mut info);
                            }
                        }

                        // Calculate distance to both attack points and choose the closer one
                        // C++ lines 823-830
                        let dist_sqr1 =
                            Self::calc_distance_squared(&source_pos, &info.attack_point1);
                        let dist_sqr2 =
                            Self::calc_distance_squared(&source_pos, &info.attack_point2);

                        if dist_sqr2 < dist_sqr1 {
                            // Use attack point 2 (closer to source)
                            actual_victim_pos = info.attack_point2;
                        } else {
                            // Use attack point 1 (closer to source, or equal distance)
                            actual_victim_pos = info.attack_point1;
                        }
                    }
                }
            }
        }

        // Calculate distance squared (C++ line 792-836)
        let dist_sqr = Self::calc_distance_squared(&source_pos, &actual_victim_pos);

        // Range checking if not ignoring ranges (C++ lines 850-886)
        if !ignore_ranges {
            let attack_range_sqr = self.get_attack_range(bonus).powi(2);
            if dist_sqr > attack_range_sqr {
                return Ok(0); // Out of max range
            }

            let min_attack_range_sqr = self.get_minimum_attack_range().powi(2);
            if dist_sqr < min_attack_range_sqr && !is_projectile_detonation {
                return Ok(0); // Too close (inside min range)
            }
        }

        // Play fire FX (C++ lines 889-941)
        // Get veterancy level from source object (C++ line 889)
        let veterancy = self.get_object_veterancy_level(source_obj);
        // Fire FX handling (C++ lines 889-941)
        // C++ calls: sourceObj->getDrawable()->handleWeaponFireFX(...)
        if let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) {
            let drawable = source_obj
                .read()
                .ok()
                .and_then(|guard| guard.get_drawable());
            if let Some(drawable) = drawable {
                if let Ok(mut draw_guard) = drawable.write() {
                    let _ = draw_guard.handle_weapon_fire_fx(
                        map_weapon_slot_to_common(weapon_slot),
                        specific_barrel_to_use,
                        &actual_victim_pos,
                    );
                }
            }
        }

        // Keep debug trace for parity diagnostics.
        log::debug!(
            "Fire FX for weapon '{}' at barrel {} (veterancy: {:?})",
            self.name,
            specific_barrel_to_use,
            veterancy
        );

        if let Some(fx) = self.get_fire_fx(veterancy) {
            if let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) {
                let _ = fx.do_fx_obj(&source_obj, None);
            } else {
                let _ = fx.do_fx_at_position(&source_pos);
            }
        }

        // Play fire OCL (C++ lines 943-950)
        if let Some(fire_ocl) = self.get_fire_ocl(veterancy) {
            // Create fire effects (muzzle flash, tracers, etc) at source position
            // C++ line 946: ObjectCreationList::create(oclToUse, sourceObj, NULL)
            if let Err(e) = fire_ocl.create_at_position(&source_pos, source_obj) {
                log::warn!(
                    "Failed to create fire OCL for weapon '{}': {}",
                    self.name,
                    e
                );
            } else {
                log::debug!("Created fire OCL for weapon '{}'", self.name);
            }
        }

        // Calculate scatter radius (C++ lines 952-996)
        let mut projectile_destination = actual_victim_pos;
        let mut scatter_radius;
        let mut target_layer = PathfindLayerEnum::Ground;

        if let Some(victim_id) = actual_victim_obj {
            if let Some(victim_obj) = TheGameLogic::find_object_by_id(victim_id) {
                if let Ok(victim_guard) = victim_obj.read() {
                    target_layer = victim_guard.get_layer();
                }
            }
        }

        // Infantry inaccuracy bonus (C++ lines 954-973):
        // infantry targets receive extra scatter from `m_infantryInaccuracyDist`.
        scatter_radius = self.effective_scatter_radius(Self::target_is_infantry(actual_victim_obj));

        if scatter_radius > 0.0 {
            // Randomize scatter (C++ lines 979-995)
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let actual_scatter = rng.gen_range(0.0..scatter_radius);
            let scatter_angle_radian = rng.gen_range(0.0..(2.0 * std::f32::consts::PI));

            projectile_destination.x += actual_scatter * scatter_angle_radian.cos();
            projectile_destination.y += actual_scatter * scatter_angle_radian.sin();
            // Get ground height at scatter destination (C++ line 995)
            // C++ code: projectileDestination.z = TheTerrainLogic->getLayerHeight(x, y, targetLayer)
            if let Some(terrain) = TheTerrainLogic::get() {
                projectile_destination.z = terrain.get_layer_height(
                    projectile_destination.x,
                    projectile_destination.y,
                    target_layer,
                );
            }

            // Clear victim object when scattering
            actual_victim_obj = None;
        }

        let current_frame = self.get_current_frame();

        // Determine weapon type and fire accordingly (C++ line 998+)
        // Three main branches: Projectile, Laser, or Instant/Delayed damage

        if !self.projectile_name.is_empty() && !is_projectile_detonation {
            // ===== PROJECTILE WEAPON (C++ lines 1077-1164) =====
            // This weapon fires a physical projectile object

            log::debug!(
                "Creating projectile '{}' from {:?} to {:?}",
                self.projectile_name,
                source_pos,
                projectile_destination
            );

            // Create the projectile object
            let proj_id = self.create_projectile(
                source_obj,
                &source_pos,
                &projectile_destination,
                bonus,
                actual_victim_obj,
                weapon_slot,
                specific_barrel_to_use,
            )?;
            *projectile_id = proj_id;

            // Notify firing weapon of new projectile (C++ line 1116)
            if let Some(firing_wpn) = firing_weapon {
                // Notify weapon that projectile was created for tracking/management
                // C++ line 1116: firingWpn->newProjectileFired(sourceID, projectileID, actualVictimID, projectileDestination)
                // NOTE: Projectile tracking system integration pending
                // When implemented, this will:
                // 1. Store projectile ID in weapon for lifetime management
                // 2. Track projectile stream continuity
                // 3. Handle multi-barrel rotation
                if let Some(new_projectile_id) = proj_id {
                    firing_wpn.new_projectile_fired(
                        source_id,
                        new_projectile_id,
                        actual_victim_obj,
                        Some(&projectile_destination),
                    );
                }
            }

            // Handle countermeasures for missiles (C++ lines 1144-1151)
            // If projectile is a missile and victim has countermeasures, activate them
            // C++ code: if (projectile->isKindOf(KINDOF_SMALL_MISSILE) && victimObj && victimObj->hasCountermeasures())
            //     victimObj->activateCountermeasures(projectileID)
            if let (Some(victim_id), Some(projectile_id)) = (actual_victim_obj, proj_id) {
                let is_missile = self.projectile_has_behavior("MissileAIUpdate")
                    || self.projectile_has_behavior("SmartBombTargetHomingUpdate");
                if is_missile {
                    if let Some(victim_arc) = TheGameLogic::find_object_by_id(victim_id) {
                        if let Ok(mut victim_guard) = victim_arc.write() {
                            for module in victim_guard.behavior_modules() {
                                if module
                                    .with_module_downcast::<CountermeasuresBehaviorModule, _, _>(
                                        |module| {
                                            let _ = module
                                                .behavior_mut()
                                                .report_missile_for_countermeasures(projectile_id);
                                        },
                                    )
                                    .is_some()
                                {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            return Ok(current_frame);
        } else if self.is_laser() {
            // ===== LASER WEAPON (C++ lines 1010-1032) =====
            // Instant-hit beam weapon with visual effect

            let should_hit_victim = scatter_radius <= self.get_primary_damage_radius(bonus)
                || scatter_radius <= self.get_secondary_damage_radius(bonus);

            let damage_id = if self.damage_dealt_at_self_position {
                INVALID_OBJECT_ID
            } else {
                victim_obj.unwrap_or(INVALID_OBJECT_ID)
            };

            if should_hit_victim {
                // Laser will track and hit the actual victim
                if let Some(vid) = victim_obj {
                    actual_victim_pos = self.get_object_position(vid)?;
                }
                // Create laser beam to target (C++ lines 1014-1020)
                // Laser objects are visual effects that persist briefly, connecting source to target
                log::debug!(
                    "Creating laser '{}' from {} to target {:?} at {:?}",
                    self.laser_name,
                    source_obj,
                    victim_obj,
                    actual_victim_pos
                );
                let _ = self.create_laser_object(source_obj, victim_obj, Some(&actual_victim_pos));
            } else {
                // Laser misses - fire at ground position (C++ lines 1022-1028)
                log::debug!(
                    "Creating laser '{}' from {} to ground at {:?}",
                    self.laser_name,
                    source_obj,
                    projectile_destination
                );
                let _ = self.create_laser_object(source_obj, None, Some(&projectile_destination));
            }

            // Apply damage immediately for lasers
            if inflict_damage {
                self.deal_damage_internal(
                    source_id,
                    damage_id,
                    &actual_victim_pos,
                    bonus,
                    is_projectile_detonation,
                )?;
            }

            return Ok(current_frame);
        } else {
            // ===== INSTANT OR DELAYED DAMAGE WEAPON (C++ lines 998-1075) =====
            // No projectile object - damage appears after flight time calculation

            let flight_vector = Coord3D::new(
                actual_victim_pos.x - source_pos.x,
                actual_victim_pos.y - source_pos.y,
                actual_victim_pos.z - source_pos.z,
            );
            let distance = flight_vector.length();

            // Calculate delay based on weapon speed (C++ line 1006)
            // Don't round - we want fractional frame delays for accuracy
            let delay_in_frames = if self.weapon_speed > 0.0 {
                distance / self.weapon_speed
            } else {
                0.0
            };

            let damage_id = if self.damage_dealt_at_self_position {
                INVALID_OBJECT_ID
            } else {
                victim_obj.unwrap_or(INVALID_OBJECT_ID)
            };

            // Determine where damage occurs
            let damage_pos = if self.damage_dealt_at_self_position {
                &source_pos
            } else {
                &actual_victim_pos
            };

            if delay_in_frames < 1.0 {
                // ===== IMMEDIATE DAMAGE (C++ lines 1036-1053) =====
                // Fast enough to apply damage this frame

                if inflict_damage {
                    self.deal_damage_internal(
                        source_id,
                        damage_id,
                        damage_pos,
                        bonus,
                        is_projectile_detonation,
                    )?;
                }

                log::debug!(
                    "Applied immediate damage from weapon '{}' (delay {:.2} frames)",
                    self.name,
                    delay_in_frames
                );

                return Ok(current_frame);
            } else {
                // ===== DELAYED DAMAGE (C++ lines 1055-1075) =====
                // Slow enough that we need to schedule damage for a future frame

                let delay_in_whole_frames = delay_in_frames.ceil() as u32;
                let when = current_frame + delay_in_whole_frames;

                if inflict_damage {
                    // Schedule on the active runtime store by template name.
                    // This keeps delayed damage alive even though this module has
                    // a parallel template type.
                    let mut scheduled = false;
                    match crate::weapon::with_weapon_store_mut(|store| {
                        if let Some(active_template) =
                            store.find_weapon_template(&self.name).cloned()
                        {
                            store.set_delayed_damage(
                                &active_template,
                                damage_pos,
                                when,
                                source_id,
                                damage_id,
                                bonus,
                            );
                            true
                        } else {
                            false
                        }
                    }) {
                        Ok(true) => {
                            scheduled = true;
                            log::debug!(
                                "Scheduled delayed damage for frame {} (delay {} frames) for weapon '{}'",
                                when,
                                delay_in_whole_frames,
                                self.name
                            );
                        }
                        Ok(false) => {
                            log::warn!(
                                "Failed to schedule delayed damage for '{}' (template not found in active store); applying immediate fallback",
                                self.name
                            );
                        }
                        Err(err) => {
                            log::warn!(
                                "Failed to schedule delayed damage for '{}' ({:?}); applying immediate fallback",
                                self.name,
                                err
                            );
                        }
                    }

                    if !scheduled {
                        self.deal_damage_internal(
                            source_id,
                            damage_id,
                            damage_pos,
                            bonus,
                            is_projectile_detonation,
                        )?;
                    }
                }

                return Ok(when);
            }
        }
    }

    /// Calculate squared distance between two positions (2D only, ignoring Z)
    ///
    /// Matches distance calculation in C++ fireWeaponTemplate
    fn calc_distance_squared(a: &Coord3D, b: &Coord3D) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dx * dx + dy * dy
    }

    /// Estimate weapon damage against target (matches C++ estimateWeaponTemplateDamage)
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

        let source_id =
            if let Some(source_arc) = crate::helpers::TheGameLogic::find_object_by_id(source_obj) {
                if let Ok(source_guard) = source_arc.read() {
                    source_guard.get_id()
                } else {
                    source_obj
                }
            } else {
                source_obj
            };

        let Some(victim_arc) = crate::helpers::TheGameLogic::find_object_by_id(victim_id) else {
            return primary_damage;
        };
        let Ok(victim_guard) = victim_arc.read() else {
            return primary_damage;
        };

        let damage_info = DamageInfoInput {
            damage_type: crate::damage::DamageType::from_u32(self.damage_type as u32),
            death_type: crate::damage::DeathType::from_u32(self.death_type as u32),
            source_id,
            amount: primary_damage,
            ..Default::default()
        };
        victim_guard.estimate_damage(&damage_info)
    }

    // ===== PRIVATE IMPLEMENTATION METHODS =====

    /// Get object position from the object manager
    ///
    /// Matches C++ TheGameLogic->findObjectByID(objID)->getPosition()
    fn get_object_position(&self, obj_id: ObjectID) -> GameLogicResult<Coord3D> {
        // Interface with object registry to get position (C++ Weapon.cpp line 790, 799)
        // C++ code: const Coord3D* sourcePos = sourceObj->getPosition()
        //
        // NOTE: Requires GameLogic singleton integration with object manager
        // The object manager maintains a hash map of all active game objects
        // and provides fast lookup by ObjectID
        //
        // Implementation plan:
        // 1. Add TheGameLogic global singleton (similar to C++)
        // 2. Implement object registry with ID-based lookup
        // 3. Return object position or ObjectNotFound error
        //
        if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            if let Ok(guard) = obj.read() {
                return Ok(*guard.get_position());
            }
        }
        Err(GameLogicError::InvalidObject(obj_id))
    }

    /// Get veterancy level from object
    ///
    /// Matches C++ sourceObject->getVeterancyLevel() from Weapon.cpp line 889
    fn get_object_veterancy_level(&self, obj_id: ObjectID) -> VeterancyLevel {
        // Interface with object system to get veterancy (C++ line 889, 903, 946)
        // C++ code: VeterancyLevel v = sourceObj->getVeterancyLevel()
        //
        // Veterancy affects:
        // 1. Which FX list to use (Regular/Veteran/Elite/Heroic effects)
        // 2. Weapon bonuses (damage, range, rate of fire multipliers)
        // 3. Visual effects quality and particle counts
        //
        // NOTE: Requires object manager integration
        // Objects track veterancy through ExperienceTracker module
        //
        if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            if let Ok(guard) = obj.read() {
                return guard.get_veterancy_level();
            }
        }
        VeterancyLevel::Regular
    }

    /// Get all objects within a radius (for area damage)
    ///
    /// Matches C++ ThePartitionManager->iterateObjectsInRange() from Weapon.cpp line 1282
    fn get_objects_in_range(
        &self,
        center: &Coord3D,
        radius: f32,
    ) -> GameLogicResult<Vec<ObjectID>> {
        // Interface with partition manager for spatial queries (C++ line 1282)
        // C++ code: iter = ThePartitionManager->iterateObjectsInRange(pos, radius, DAMAGE_RANGE_CALC_TYPE)
        //
        // DAMAGE_RANGE_CALC_TYPE = FROM_BOUNDINGSPHERE_3D (C++ line 70)
        // Uses full 3D distance including height for damage calculations.
        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(Vec::new());
        };

        Ok(partition.get_objects_in_range_boundary_3d(center, radius))
    }

    /// Check if object should be affected by this weapon
    ///
    /// Matches C++ filtering logic from Weapon.cpp lines 1291-1309
    fn should_affect_object(
        &self,
        source_id: ObjectID,
        target_id: ObjectID,
        affects_mask: u32,
    ) -> bool {
        let Some(target) = crate::helpers::TheGameLogic::find_object_by_id(target_id) else {
            return false;
        };
        let Ok(target_guard) = target.read() else {
            return false;
        };
        if target_guard.is_destroyed() {
            return false;
        }

        let Some(source) = crate::helpers::TheGameLogic::find_object_by_id(source_id) else {
            if (affects_mask & WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE) != 0
                && target_guard.is_significantly_above_terrain()
            {
                return false;
            }
            return true;
        };
        let Ok(source_guard) = source.read() else {
            return false;
        };

        if (affects_mask & WeaponAffectsMask::SELF) == 0 {
            if source_id == target_id || source_guard.get_producer_id() == target_id {
                return false;
            }
        }

        if (affects_mask & WeaponAffectsMask::DOESNT_AFFECT_SIMILAR) != 0 {
            let relationship = target_guard.relationship_to(&source_guard);
            if matches!(
                relationship,
                Relationship::Ally | Relationship::Allies | Relationship::Friend
            ) {
                if source_guard
                    .get_template()
                    .is_equivalent_to(target_guard.get_template().as_ref())
                {
                    return false;
                }
            }
        }

        if (affects_mask & WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE) != 0
            && target_guard.is_significantly_above_terrain()
        {
            return false;
        }

        let relationship = target_guard.relationship_to(&source_guard);
        let required_mask = match relationship {
            Relationship::Ally | Relationship::Allies | Relationship::Friend => {
                WeaponAffectsMask::ALLIES
            }
            Relationship::Enemy => WeaponAffectsMask::ENEMIES,
            _ => WeaponAffectsMask::NEUTRALS,
        };

        (affects_mask & required_mask) != 0
    }

    /// Apply damage to a specific object
    ///
    /// Matches C++ damage application from Weapon.cpp lines 1378-1438
    fn apply_damage_to_object(
        &self,
        target_id: ObjectID,
        source_id: ObjectID,
        damage_amount: f32,
        damage_type: DamageType,
        death_type: DeathType,
        damage_status: ObjectStatusTypes,
        damage_pos: &Coord3D,
        target_pos: &Coord3D,
    ) -> GameLogicResult<()> {
        // Build DamageInfo and apply to object (C++ lines 1378-1438)
        // C++ code: target->attemptDamage(&damageInfo)
        //
        // DamageInfo structure contains:
        // Input fields (set by weapon):
        // - source_id: ObjectID of attacker
        // - damage_amount: Base damage before armor/resistance
        // - damage_type: Type for armor calculation (explosion, small_arms, etc)
        // - death_type: How unit dies (burned, crushed, exploded, etc)
        // - damage_status: Status effects to apply (burning, poisoned, etc)
        // - shock_wave: Physics impulse vector and strength
        //
        // Output fields (filled by target):
        // - actual_damage: Damage after armor/resistance
        // - killed: Whether target was destroyed
        // - veterancy_gained: Experience awarded to attacker
        //
        // Damage application flow:
        // 1. Look up target object by ID
        // 2. Build DamageInfo with all weapon parameters
        // 3. Calculate shockwave if weapon has knockback
        // 4. Call target->attemptDamage() which:
        //    - Applies armor/resistance modifiers
        //    - Reduces current health
        //    - Triggers death if health <= 0
        //    - Awards veterancy to source
        //    - Plays damage FX and sounds
        //
        // NOTE: Requires:
        // 1. Object manager for target lookup
        // 2. DamageInfo system (GameLogic/Damage.h)
        // 3. BodyModule for health management
        // 4. ExperienceTracker for veterancy
        //
        let Some(target_obj) = crate::helpers::TheGameLogic::find_object_by_id(target_id) else {
            return Err(GameLogicError::InvalidObject(target_id));
        };

        let mut damage_info = DamageInfo::default();
        damage_info.input.source_id = source_id;
        damage_info.input.damage_type = crate::damage::DamageType::from_u32(damage_type as u32);
        damage_info.input.death_type = crate::damage::DeathType::from_u32(death_type as u32);
        damage_info.input.damage_status_type = damage_status;
        damage_info.input.amount = damage_amount;

        if let Some(source_obj) = crate::helpers::TheGameLogic::find_object_by_id(source_id) {
            if let Ok(source_guard) = source_obj.read() {
                damage_info.input.source_template = Some(source_guard.get_template().clone());
                if let Some(player_id) = source_guard.get_controlling_player_id() {
                    let bit = if player_id < 8 { 1u32 << player_id } else { 0 };
                    damage_info.input.source_player_mask = PlayerMaskType::from_bits_truncate(bit);
                }
            }
        }

        // Calculate shockwave vector if needed (C++ lines 1413-1428)
        if self.shock_wave_amount > 0.0 {
            let shock_vec = Coord3D::new(
                target_pos.x - damage_pos.x,
                target_pos.y - damage_pos.y,
                target_pos.z - damage_pos.z,
            );
            damage_info.input.shock_wave_vector = shock_vec;
            damage_info.input.shock_wave_amount = self.shock_wave_amount;
            damage_info.input.shock_wave_radius = self.shock_wave_radius;
            damage_info.input.shock_wave_taper_off = self.shock_wave_taper_off;
        }

        damage_info.sync_from_input();
        if let Ok(mut guard) = target_obj.write() {
            let _ = guard.attempt_damage(&mut damage_info);
        }

        Ok(())
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

    /// Deal damage immediately with full radius and armor calculations
    ///
    /// Matches C++ WeaponTemplate::dealDamageInternal() from Weapon.cpp line 1197
    fn deal_damage_internal(
        &self,
        source_obj: ObjectID,
        victim_obj: ObjectID,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
    ) -> GameLogicResult<()> {
        // C++ line 1199-1203: Validation
        if source_obj == 0 {
            return Ok(()); // Must have a source
        }
        if victim_obj == 0 && target_pos == &Coord3D::new(0.0, 0.0, 0.0) {
            return Ok(()); // Must have some sort of destination
        }

        // Get actual damage position from victim if specified (C++ lines 1256-1260)
        let actual_pos = if victim_obj != 0 && victim_obj != INVALID_OBJECT_ID {
            // Get victim's current position for accurate damage application
            // C++ code: primaryVictim = TheGameLogic->findObjectByID(victimID)
            //           pos = primaryVictim->getPosition()
            //
            // Important for:
            // - Moving targets (position may have changed since fire decision)
            // - Tracking targets (missiles following target)
            // - Area damage centering (explosion at victim, not original position)
            self.get_object_position(victim_obj).unwrap_or(*target_pos)
        } else {
            *target_pos
        };

        // Historic bonus weapon system (C++ lines 1214-1251)
        // Track recent hits and optionally trigger a chained bonus weapon.
        if self.historic_bonus_count > 0 && self.historic_bonus_weapon.is_some() {
            let current_frame = self.get_current_frame();
            self.record_historic_damage(&actual_pos, current_frame);

            if let Some(bonus_weapon) = self.check_historic_bonus(current_frame, &actual_pos) {
                if !std::ptr::eq(self, bonus_weapon.as_ref()) {
                    let mut bonus_projectile_id = None;
                    let bonus_victim = if victim_obj != 0 && victim_obj != INVALID_OBJECT_ID {
                        Some(victim_obj)
                    } else {
                        None
                    };

                    if let Err(e) = bonus_weapon.fire_weapon_template(
                        source_obj,
                        WeaponSlotType::Primary,
                        0,
                        bonus_victim,
                        Some(&actual_pos),
                        bonus,
                        false,
                        true,
                        None,
                        &mut bonus_projectile_id,
                        true,
                    ) {
                        log::warn!(
                            "Failed to fire historic bonus weapon '{}' from '{}': {}",
                            bonus_weapon.name,
                            self.name,
                            e
                        );
                    }
                }
            }
        }

        // Get weapon properties (C++ lines 1262-1275)
        let damage_type = self.damage_type;
        let death_type = self.death_type;
        let damage_status_type = self.damage_status_type;

        let primary_damage = self.get_primary_damage(bonus);
        let primary_radius = self.get_primary_damage_radius(bonus);
        let secondary_damage = self.get_secondary_damage(bonus);
        let secondary_radius = self.get_secondary_damage_radius(bonus);
        let affects_mask = self.affects_mask.bits();

        log::debug!(
            "Weapon '{}' dealing damage: Primary={:.1} (radius={:.1}), Secondary={:.1} (radius={:.1}) at {:?}",
            self.name,
            primary_damage,
            primary_radius,
            secondary_damage,
            secondary_radius,
            actual_pos
        );

        // C++ line 1277: Validate radius ordering
        debug_assert!(
            secondary_radius >= primary_radius || secondary_radius == 0.0,
            "Secondary radius should be >= primary radius (or zero)"
        );

        let primary_radius_sqr = primary_radius * primary_radius;
        let radius = primary_radius.max(secondary_radius);

        // Iterate over all objects in damage radius (C++ lines 1281-1311)
        if radius > 0.0 {
            // ===== AREA EFFECT DAMAGE =====
            // Get objects in damage radius (C++ line 1282)
            let objects_in_range = self.get_objects_in_range(&actual_pos, radius)?;

            log::debug!(
                "Area damage weapon '{}' affecting {} objects in radius {:.1} from {:?}",
                self.name,
                objects_in_range.len(),
                radius,
                actual_pos
            );

            // Apply damage to each object in range (C++ lines 1284-1438)
            for target_id in objects_in_range {
                // Skip invalid objects
                if target_id == 0 || target_id == INVALID_OBJECT_ID {
                    continue;
                }

                // Skip source object unless WEAPON_KILLS_SELF is set
                if target_id == source_obj {
                    if (affects_mask & WeaponAffectsMask::KILLS_SELF) == 0 {
                        continue;
                    }
                }

                // Check if this object should be affected by this weapon
                // C++ lines 1291-1309: affects mask, relationship, kindof checks
                if !self.should_affect_object(source_obj, target_id, affects_mask) {
                    continue;
                }

                // Get target position for distance calculation
                let target_pos = match self.get_object_position(target_id) {
                    Ok(pos) => pos,
                    Err(_) => continue, // Skip if we can't get position
                };

                // ===== RADIUS DAMAGE ANGLE CONE CHECK (C++ lines 1389-1400) =====
                // If radius_damage_angle < PI, damage is constrained to a directional cone
                let allowed_angle = self.radius_damage_angle;
                if allowed_angle < std::f32::consts::PI {
                    // Directional cone damage - only affect targets in front of the source
                    // Get source object to determine facing direction
                    let Some(source_obj_arc) =
                        crate::helpers::TheGameLogic::find_object_by_id(source_obj)
                    else {
                        continue; // Can't determine source direction, bail
                    };
                    let Ok(source_guard) = source_obj_arc.read() else {
                        continue; // Can't read source, bail
                    };

                    // Get source's forward direction vector (X-axis from transform)
                    // C++ code: Vector3 sourceVector = source->getTransformMatrix()->Get_X_Vector()
                    let source_angle = source_guard.get_geometry_info().angle;
                    let source_dir = Coord3D::new(source_angle.cos(), source_angle.sin(), 0.0);

                    // Calculate damage direction vector (from source position to target)
                    // C++ code: damageDirection.set(curVictim->getPosition()); damageDirection.sub(source->getPosition())
                    let source_pos = source_guard.get_position();
                    let damage_dir = Coord3D::new(
                        target_pos.x - source_pos.x,
                        target_pos.y - source_pos.y,
                        target_pos.z - source_pos.z,
                    );

                    // Normalize both vectors for dot product calculation
                    let source_dir_norm = source_dir.normalize();
                    let damage_dir_norm = damage_dir.normalize();

                    // Dot product gives cos(angle between vectors)
                    // C++ code: if( Vector3::Dot_Product(sourceVector, damageVector) < Cos(allowedAngle) )
                    let dot = source_dir_norm.x * damage_dir_norm.x
                        + source_dir_norm.y * damage_dir_norm.y
                        + source_dir_norm.z * damage_dir_norm.z;

                    // If dot < cos(allowed_angle), target is outside the cone
                    if dot < allowed_angle.cos() {
                        continue; // Too far to the side, can't hurt them
                    }
                }

                // Calculate distance from damage center
                let dist_sqr = Self::calc_distance_squared(&actual_pos, &target_pos);

                // Determine if target is in primary or secondary damage radius
                let damage_amount = if dist_sqr <= primary_radius_sqr {
                    primary_damage // Full primary damage
                } else {
                    secondary_damage // Secondary damage for outer radius
                };

                // Skip if no damage to deal
                if damage_amount <= 0.0 {
                    continue;
                }

                // Apply damage to this target (C++ lines 1378-1438)
                if let Err(e) = self.apply_damage_to_object(
                    target_id,
                    source_obj,
                    damage_amount,
                    damage_type,
                    death_type,
                    damage_status_type,
                    &actual_pos,
                    &target_pos,
                ) {
                    log::warn!("Failed to apply damage to object {}: {}", target_id, e);
                }
            }
        } else if victim_obj != 0 && victim_obj != INVALID_OBJECT_ID {
            // ===== SINGLE TARGET DAMAGE (C++ lines 1286-1307) =====
            // No radius - damage only the specific victim

            log::debug!(
                "Single-target weapon '{}' dealing {:.1} damage to object {}",
                self.name,
                primary_damage,
                victim_obj
            );

            // Apply damage to single target (C++ lines 1378-1438)
            if let Err(e) = self.apply_damage_to_object(
                victim_obj,
                source_obj,
                primary_damage,
                damage_type,
                death_type,
                damage_status_type,
                &actual_pos,
                &actual_pos, // target_pos same as damage_pos for single target
            ) {
                log::warn!("Failed to apply damage to object {}: {}", victim_obj, e);
            }
        } else {
            // No radius and no victim - check for special WEAPON_KILLS_SELF flag
            // Suicide weapons (demo traps, IED vehicles, etc)
            if (affects_mask & WeaponAffectsMask::KILLS_SELF) != 0 {
                // Self-destruct weapon (C++ uses HUGE_DAMAGE_AMOUNT)
                // C++ code: damageInfo.in.m_amount = HUGE_DAMAGE_AMOUNT (typically 9999.0)
                //           source->attemptDamage(&damageInfo)
                //
                // Used for:
                // - Demo trap explosions (kill self to trigger bomb)
                // - Suicide vehicles (GLA bomb truck)
                // - Self-destruct abilities
                //
                // NOTE: Requires object manager integration
                log::debug!(
                    "Weapon '{}' killing source object {} (WEAPON_KILLS_SELF)",
                    self.name,
                    source_obj
                );
                if let Err(e) = self.apply_damage_to_object(
                    source_obj,
                    source_obj,
                    crate::damage::HUGE_DAMAGE_AMOUNT,
                    self.damage_type,
                    self.death_type,
                    self.damage_status_type,
                    &actual_pos,
                    &actual_pos,
                ) {
                    log::warn!(
                        "Failed to apply WEAPON_KILLS_SELF damage for source {}: {}",
                        source_obj,
                        e
                    );
                }
            }
        }

        if is_projectile_detonation {
            let veterancy = self.get_object_veterancy_level(source_obj);
            if let Some(fx) = self.get_projectile_detonate_fx(veterancy) {
                let _ = fx.do_fx_at_position_with_radius(&actual_pos, self.primary_damage_radius);
            }
            if let Some(ocl) = self.get_projectile_detonation_ocl(veterancy) {
                let _ = ocl.create_at_position(&actual_pos, source_obj);
            }
        }

        Ok(())
    }

    /// Calculate projectile flight time
    fn calculate_projectile_flight_time(
        &self,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
    ) -> GameLogicResult<u32> {
        let distance = source_pos.distance(*target_pos);
        let speed = if self.is_scale_weapon_speed {
            // Scale speed based on range (for lobbing weapons)
            let max_range = self.get_unmodified_attack_range();
            let speed_scale = if max_range > 0.0 {
                distance / max_range
            } else {
                1.0
            };
            let scaled_speed =
                self.min_weapon_speed + (self.weapon_speed - self.min_weapon_speed) * speed_scale;
            scaled_speed.max(self.min_weapon_speed)
        } else {
            self.weapon_speed
        };

        if speed <= 0.0 {
            return Ok(1); // Immediate hit for zero or negative speed
        }

        // Calculate flight time in frames
        let flight_time_seconds = distance / speed;
        let flight_time_frames = (flight_time_seconds * LOGICFRAMES_PER_SECOND as f32) as u32;

        Ok(flight_time_frames.max(1)) // Minimum 1 frame
    }

    /// Create projectile object
    fn create_projectile(
        &self,
        source_obj: ObjectID,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
        victim_obj: Option<ObjectID>,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
    ) -> GameLogicResult<Option<ObjectID>> {
        if self.projectile_name.is_empty() {
            return Ok(None);
        }

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

            if let Some(player_arc) = owning_player {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.get_num_battle_plans_active() > 0 {
                        if let Ok(mut proj_guard) = projectile_arc.write() {
                            player_guard.apply_battle_plan_bonuses_for_object(&mut proj_guard);
                        }
                    }
                }
            }

            let exhaust = self
                .get_projectile_exhaust(source_veterancy)
                .map(|tmpl| Arc::new(tmpl.clone()));

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
                                    None,
                                    exhaust.clone(),
                                );
                                did_launch = true;
                            }
                            ProjectileLaunchKindMut::NeutronMissileUpdate(neutron) => {
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
                                                            None,
                                                            None,
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
                                                None,
                                                None,
                                            );
                                            did_launch = true;
                                        }
                                    }
                                }
                            }
                            ProjectileLaunchKindMut::DumbProjectileBehavior(dumb) => {
                                dumb.projectile_launch_at_object_or_position(
                                    victim_obj, target_pos, source_obj, None,
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

            return Ok(Some(projectile_id));
        }

        Err(GameLogicError::Configuration(format!(
            "Projectile template '{}' not found",
            self.projectile_name
        )))
    }

    /// Create laser object
    fn create_laser_object(
        &self,
        source_obj: ObjectID,
        victim_obj: Option<ObjectID>,
        victim_pos: Option<&Coord3D>,
    ) -> GameLogicResult<Option<ObjectID>> {
        if self.laser_name.is_empty() {
            return Ok(None);
        }

        log::debug!("Creating laser object '{}'", self.laser_name);

        let Some(source_arc) = crate::helpers::TheGameLogic::find_object_by_id(source_obj) else {
            return Err(GameLogicError::InvalidObject(source_obj));
        };
        let (team_arc, source_pos) = {
            let source_guard = source_arc.read().map_err(|_| {
                GameLogicError::Threading("Failed to lock source object".to_string())
            })?;
            let Some(team_arc) = source_guard.get_team() else {
                return Err(GameLogicError::Configuration(
                    "Laser creation requires a source team".to_string(),
                ));
            };
            (team_arc, *source_guard.get_position())
        };
        let team_guard = team_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock source team".to_string()))?;

        let Some(template) = crate::helpers::TheThingFactory::find_template(&self.laser_name)
        else {
            return Err(GameLogicError::Configuration(format!(
                "Laser template '{}' not found",
                self.laser_name
            )));
        };

        let factory = crate::helpers::TheThingFactory::get()
            .map_err(|e| GameLogicError::SystemNotInitialized(e.to_string()))?;
        let laser_obj = factory
            .new_object(template, &team_guard)
            .map_err(|e| GameLogicError::ModuleError(e.to_string()))?;

        let mut laser_guard = laser_obj
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock laser object".to_string()))?;
        let end_pos = if let Some(pos) = victim_pos {
            *pos
        } else if let Some(target_id) = victim_obj {
            crate::helpers::TheGameLogic::find_object_by_id(target_id)
                .and_then(|target_arc| target_arc.read().ok().map(|guard| *guard.get_position()))
                .unwrap_or(source_pos)
        } else {
            source_pos
        };
        let _ = laser_guard.set_position(&end_pos);
        let laser_id = laser_guard.get_id();

        let client_modules =
            laser_guard.drawable_modules_with_interface(ModuleInterfaceType::CLIENT_UPDATE);
        drop(laser_guard);

        let source_guard = source_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock source object".to_string()))?;
        let target_arc = victim_obj.and_then(crate::helpers::TheGameLogic::find_object_by_id);
        let target_guard = match target_arc.as_ref() {
            Some(target_arc) => target_arc.read().ok(),
            None => None,
        };
        let target_ref = target_guard.as_deref();
        let end_pos_ref = victim_pos.or(if victim_obj.is_some() {
            None
        } else {
            Some(&end_pos)
        });

        for module in client_modules {
            let _ = module.with_module_downcast::<crate::object::update::LaserUpdateModule, _, _>(
                |laser_update| {
                    laser_update.update_mut().init_laser(
                        Some(&*source_guard),
                        target_ref,
                        None,
                        end_pos_ref,
                        self.laser_bone_name.clone(),
                        0,
                    );
                },
            );
        }

        Ok(Some(laser_id))
    }

    /// Apply immediate firing effects
    fn apply_firing_effects(
        &self,
        source_obj: ObjectID,
        source_pos: &Coord3D,
        weapon_slot: WeaponSlotType,
        bonus: &WeaponBonus,
    ) -> GameLogicResult<()> {
        // 1. Play fire sound
        if !self.fire_sound.is_empty() {
            log::debug!("Playing fire sound for weapon '{}'", self.name);
            // Sound system integration would go here
        }

        // 2. Apply weapon recoil to source object
        if self.weapon_recoil > 0.0 {
            log::debug!("Applying recoil {} to source object", self.weapon_recoil);
            // Physics system integration would go here
        }

        // 3. Trigger fire effects based on veterancy
        // This would get veterancy from source object and trigger appropriate FX

        // 4. Handle suspended FX timing
        if self.suspend_fx_delay > 0 {
            // Schedule FX to play after delay
        }

        Ok(())
    }

    /// Handle scatter targets
    fn handle_scatter_targets(
        &self,
        source_obj: ObjectID,
        primary_target_pos: &Coord3D,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> GameLogicResult<()> {
        for scatter_target in &self.scatter_targets {
            let scatter_pos = Coord3D::new(
                primary_target_pos.x + scatter_target.x * self.scatter_target_scalar,
                primary_target_pos.y + scatter_target.y * self.scatter_target_scalar,
                primary_target_pos.z,
            );

            // Fire at scatter position
            if inflict_damage {
                self.deal_damage_internal(
                    source_obj,
                    INVALID_OBJECT_ID,
                    &scatter_pos,
                    bonus,
                    false,
                )?;
            }
        }

        Ok(())
    }

    /// Process request assistance (call nearby units to join attack)
    fn process_request_assistance(
        &self,
        source_obj: ObjectID,
        victim_obj: Option<ObjectID>,
        target_pos: &Coord3D,
    ) -> GameLogicResult<()> {
        // This would find nearby friendly units within request_assist_range
        // and order them to attack the same target
        log::debug!(
            "Requesting assistance within range {} for attack on {:?}",
            self.request_assist_range,
            target_pos
        );

        Ok(())
    }

    /// Get current game frame
    fn get_current_frame(&self) -> u32 {
        crate::helpers::TheGameLogic::get_frame()
    }

    /// Record historic damage for bonus calculations
    fn record_historic_damage(&self, location: &Coord3D, frame: u32) {
        if self.historic_bonus_time > 0 {
            if let Ok(mut damage_list) = self.historic_damage.lock() {
                let damage_info = HistoricWeaponDamageInfo::new(frame, *location);
                damage_list.push_back(damage_info);
            }

            // Keep only recent damage entries within the bonus time window
            self.trim_old_historic_damage(frame);
        }
    }

    /// Trim old historic damage entries (matches C++ trimOldHistoricDamage)
    fn trim_old_historic_damage(&self, current_frame: u32) {
        if let Ok(mut damage_list) = self.historic_damage.lock() {
            let cutoff_frame = current_frame.saturating_sub(self.historic_bonus_time);
            while let Some(front) = damage_list.front() {
                if front.frame < cutoff_frame {
                    damage_list.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Check if historic bonus weapon should fire
    pub fn check_historic_bonus(
        &self,
        current_frame: u32,
        location: &Coord3D,
    ) -> Option<Arc<WeaponTemplate>> {
        if self.historic_bonus_count <= 0 || self.historic_bonus_time == 0 {
            return None;
        }

        if let Ok(damage_list) = self.historic_damage.lock() {
            // Count recent damage events within radius
            let cutoff_frame = current_frame.saturating_sub(self.historic_bonus_time);
            let count = damage_list
                .iter()
                .filter(|info| {
                    info.frame >= cutoff_frame
                        && info.location.distance(*location) <= self.historic_bonus_radius
                })
                .count();

            if count as i32 >= self.historic_bonus_count {
                if let Some(bonus_weapon) = &self.historic_bonus_weapon {
                    return bonus_weapon.upgrade();
                }
            }
        }

        None
    }

    // ===== POST PROCESSING =====

    /// Post-process load (resolve references, validate data)
    pub fn post_process_load(&mut self) -> GameLogicResult<()> {
        // This would:
        // 1. Resolve projectile template references
        // 2. Resolve historic bonus weapon references
        // 3. Validate all numeric ranges
        // 4. Set up FX and OCL references
        // 5. Initialize any computed values

        Ok(())
    }

    // ===== SETTERS FOR TESTING =====

    /// Set clip size
    pub fn set_clip_size(&mut self, clip_size: i32) {
        self.clip_size = clip_size;
    }

    /// Set auto reloads clip
    pub fn set_auto_reloads_clip(&mut self, auto_reloads: bool) {
        self.reload_type = if auto_reloads {
            WeaponReloadType::AutoReload
        } else {
            WeaponReloadType::NoReload
        };
    }

    /// Set delay between shots
    pub fn set_delay_between_shots(&mut self, delay: i32) {
        self.min_delay_between_shots = delay;
        self.max_delay_between_shots = delay;
    }

    /// Set attack range
    pub fn set_attack_range(&mut self, range: f32) {
        self.attack_range = range;
    }

    /// Set minimum attack range
    pub fn set_minimum_attack_range(&mut self, range: f32) {
        self.minimum_attack_range = range;
    }

    // -----------------------------------------------------------------------
    // INI field parsing -- mirrors C++ TheWeaponTemplateFieldParseTable
    //
    // Each field here corresponds to an entry in the C++ field parse table
    // defined in Weapon.cpp lines 143-224.
    // -----------------------------------------------------------------------

    /// Apply parsed INI key=value properties to this weapon template.
    ///
    /// C++ Reference: WeaponTemplate::TheWeaponTemplateFieldParseTable (Weapon.cpp:143-224)
    pub fn parse_weapon_fields_from_ini(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            let trimmed = value.trim();
            match key.as_str() {
                // --- Damage ---
                "PrimaryDamage" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.primary_damage = v; }
                }
                "PrimaryDamageRadius" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.primary_damage_radius = v; }
                }
                "SecondaryDamage" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.secondary_damage = v; }
                }
                "SecondaryDamageRadius" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.secondary_damage_radius = v; }
                }
                "ShockWaveAmount" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.shock_wave_amount = v; }
                }
                "ShockWaveRadius" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.shock_wave_radius = v; }
                }
                "ShockWaveTaperOff" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.shock_wave_taper_off = v; }
                }

                // --- Range & targeting ---
                "AttackRange" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.attack_range = v; }
                }
                "MinimumAttackRange" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.minimum_attack_range = v; }
                }
                "RequestAssistRange" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.request_assist_range = v; }
                }
                "AcceptableAimDelta" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.aim_delta = v; }
                }
                "ScatterRadius" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.scatter_radius = v; }
                }
                "ScatterTargetScalar" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.scatter_target_scalar = v; }
                }
                "ScatterRadiusVsInfantry" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.infantry_inaccuracy_dist = v; }
                }

                // --- Damage & death types ---
                "DamageType" => {
                    // C++: DamageTypeFlags::parseSingleBitFromINI
                    self.damage_type = parse_damage_type(trimmed);
                }
                "DamageStatusType" => {
                    // C++: ObjectStatusMaskType::parseSingleBitFromINI
                    self.damage_status_type = parse_damage_status_type(trimmed);
                }
                "DeathType" => {
                    self.death_type = parse_death_type(trimmed);
                }

                // --- Speed ---
                "WeaponSpeed" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.weapon_speed = v; }
                }
                "MinWeaponSpeed" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.min_weapon_speed = v; }
                }
                "ScaleWeaponSpeed" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.is_scale_weapon_speed = v; }
                }

                // --- Angles ---
                "WeaponRecoil" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.weapon_recoil = v; }
                }
                "MinTargetPitch" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.min_target_pitch = v; }
                }
                "MaxTargetPitch" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.max_target_pitch = v; }
                }
                "RadiusDamageAngle" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.radius_damage_angle = v; }
                }

                // --- Projectile ---
                "ProjectileObject" => {
                    self.projectile_name = trimmed.to_string();
                }
                "ProjectileStreamName" => {
                    self.projectile_stream_name = trimmed.to_string();
                }
                "LaserName" => {
                    self.laser_name = trimmed.to_string();
                }
                "LaserBoneName" => {
                    self.laser_bone_name = trimmed.to_string();
                }

                // --- Timing ---
                "ClipSize" => {
                    if let Ok(v) = trimmed.parse::<i32>() { self.clip_size = v; }
                }
                "ClipReloadTime" => {
                    // C++: parseDurationUnsignedInt -> frames at 30 FPS
                    if let Ok(v) = trimmed.parse::<u32>() { self.clip_reload_time = v as i32; }
                }
                "AutoReloadWhenIdle" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.auto_reload_when_idle_frames = v; }
                }
                "ShotsPerBarrel" => {
                    if let Ok(v) = trimmed.parse::<i32>() { self.shots_per_barrel = v; }
                }
                "PreAttackDelay" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.pre_attack_delay = v as i32; }
                }
                "SuspendFXDelay" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.suspend_fx_delay = v; }
                }

                // --- Continuous fire ---
                "ContinuousFireOne" => {
                    if let Ok(v) = trimmed.parse::<i32>() { self.continuous_fire_one_shots_needed = v; }
                }
                "ContinuousFireTwo" => {
                    if let Ok(v) = trimmed.parse::<i32>() { self.continuous_fire_two_shots_needed = v; }
                }
                "ContinuousFireCoast" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.continuous_fire_coast_frames = v; }
                }

                // --- Special targeting ---
                "ContinueAttackRange" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.continue_attack_range = v; }
                }

                // --- Flags ---
                "DamageDealtAtSelfPosition" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.damage_dealt_at_self_position = v; }
                }
                "LeechRangeWeapon" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.leech_range_weapon = v; }
                }
                "CapableOfFollowingWaypoints" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.capable_of_following_waypoint = v; }
                }
                "ShowsAmmoPips" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.is_shows_ammo_pips = v; }
                }
                "AllowAttackGarrisonedBldgs" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.allow_attack_garrisoned_bldgs = v; }
                }
                "PlayFXWhenStealthed" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.play_fx_when_stealthed = v; }
                }
                "MissileCallsOnDie" => {
                    if let Ok(v) = parse_bool_simple(trimmed) { self.die_on_detonate = v; }
                }

                // --- Anti-mask ---
                "AntiGround" => {
                    self.anti_mask.insert(WeaponAntiMask::GROUND);
                }
                "AntiAirborneVehicle" => {
                    self.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
                }
                "AntiProjectile" => {
                    self.anti_mask.insert(WeaponAntiMask::PROJECTILE);
                }
                "AntiSmallMissile" => {
                    self.anti_mask.insert(WeaponAntiMask::SMALL_MISSILE);
                }
                "AntiMine" => {
                    self.anti_mask.insert(WeaponAntiMask::MINE);
                }
                "AntiParachute" => {
                    self.anti_mask.insert(WeaponAntiMask::PARACHUTE);
                }
                "AntiAirborneInfantry" => {
                    self.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
                }
                "AntiBallisticMissile" => {
                    self.anti_mask.insert(WeaponAntiMask::BALLISTIC_MISSILE);
                }

                // --- Reload type ---
                "AutoReloadsClip" => {
                    self.reload_type = parse_weapon_reload_type(trimmed);
                }

                // --- Pre-fire type ---
                "PreAttackType" => {
                    self.prefire_type = parse_weapon_prefire_type(trimmed);
                }

                // --- Historic bonus ---
                "HistoricBonusTime" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.historic_bonus_time = v; }
                }
                "HistoricBonusRadius" => {
                    if let Ok(v) = trimmed.parse::<f32>() { self.historic_bonus_radius = v; }
                }
                "HistoricBonusCount" => {
                    if let Ok(v) = trimmed.parse::<i32>() { self.historic_bonus_count = v; }
                }

                // --- Fire sound ---
                "FireSound" => {
                    self.fire_sound = AudioEventRts::new(trimmed.to_string());
                }
                "FireSoundLoopTime" => {
                    if let Ok(v) = trimmed.parse::<u32>() { self.fire_sound_loop_time = v; }
                }

                // --- DelayBetweenShots ---
                "DelayBetweenShots" => {
                    // C++: parseShotDelay reads 1 or 2 integers
                    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                    if let Ok(v) = tokens.first().unwrap_or(&"0").parse::<i32>() {
                        self.min_delay_between_shots = v;
                    }
                    if let Some(second) = tokens.get(1) {
                        if let Ok(v) = second.parse::<i32>() {
                            self.max_delay_between_shots = v;
                        } else {
                            self.max_delay_between_shots = self.min_delay_between_shots;
                        }
                    } else {
                        self.max_delay_between_shots = self.min_delay_between_shots;
                    }
                }

                // --- Radius damage affects / collide ---
                "RadiusDamageAffects" => {
                    // C++: INI::parseBitString32 with TheWeaponAffectsMaskNames
                    // Pipe-separated list of flag names
                    for token in trimmed.split('|') {
                        let t = token.trim().to_ascii_uppercase();
                        match t.as_str() {
                            "SELF" => self.affects_mask.insert(WeaponAffectsMask::SELF),
                            "ALLIES" => self.affects_mask.insert(WeaponAffectsMask::ALLIES),
                            "ENEMIES" => self.affects_mask.insert(WeaponAffectsMask::ENEMIES),
                            "NEUTRALS" => self.affects_mask.insert(WeaponAffectsMask::NEUTRALS),
                            "KILLS_SELF" => self.affects_mask.insert(WeaponAffectsMask::KILLS_SELF),
                            "DOESNT_AFFECT_SIMILAR" => self.affects_mask.insert(WeaponAffectsMask::DOESNT_AFFECT_SIMILAR),
                            "DOESNT_AFFECT_AIRBORNE" => self.affects_mask.insert(WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE),
                            "NOT_AIRBORNE" => self.affects_mask.insert(WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE),
                            _ => {}
                        }
                    }
                }
                "ProjectileCollidesWith" => {
                    // C++: INI::parseBitString32 with TheWeaponCollideMaskNames
                    for token in trimmed.split('|') {
                        let t = token.trim().to_ascii_uppercase();
                        match t.as_str() {
                            "ALLIES" => self.collide_mask.insert(WeaponCollideMask::ALLIES),
                            "ENEMIES" => self.collide_mask.insert(WeaponCollideMask::ENEMIES),
                            "STRUCTURES" => self.collide_mask.insert(WeaponCollideMask::STRUCTURES),
                            "SHRUBBERY" => self.collide_mask.insert(WeaponCollideMask::SHRUBBERY),
                            "PROJECTILES" => self.collide_mask.insert(WeaponCollideMask::PROJECTILE),
                            "WALLS" => self.collide_mask.insert(WeaponCollideMask::WALLS),
                            "SMALL_MISSILES" => self.collide_mask.insert(WeaponCollideMask::SMALL_MISSILES),
                            "BALLISTIC_MISSILES" => self.collide_mask.insert(WeaponCollideMask::BALLISTIC_MISSILES),
                            _ => {}
                        }
                    }
                }

                // --- WeaponBonus sub-block handled separately ---
                "WeaponBonus" | "ScatterTarget" => {}

                // Everything else: silently skip
                _ => {}
            }
        }
    }
}

impl Default for WeaponTemplate {
    fn default() -> Self {
        Self::new(String::new())
    }
}

// ---------------------------------------------------------------------------
// Weapon INI field parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool_simple(s: &str) -> Result<bool, ()> {
    match s {
        "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "1" => Ok(true),
        "no" | "No" | "NO" | "false" | "False" | "FALSE" | "0" => Ok(false),
        _ => Err(()),
    }
}

fn parse_damage_type(s: &str) -> DamageType {
    match s.trim().to_ascii_uppercase().as_str() {
        "UNRESISTABLE" => DamageType::Unresistable,
        "EXPLOSION" => DamageType::Explosion,
        "CRUSH" => DamageType::Crush,
        "SMALL_ARMS" | "SMALLARMS" => DamageType::SmallArms,
        "FLAME" => DamageType::Flame,
        "LASER" => DamageType::Laser,
        "TOXIN" | "POISON" => DamageType::Poison,
        "RADIATION" => DamageType::Radiation,
        "PARTICLE_BEAM" | "PARTICLEBEAM" => DamageType::ParticleBeam,
        "HEALING" => DamageType::Healing,
        "ARMOR_PIERCING" | "ARMORPIERCING" => DamageType::ArmorPiercing,
        "GATTLING" => DamageType::Gattling,
        "SNIPER" => DamageType::Sniper,
        "WATER" => DamageType::Water,
        "DEPLOY" => DamageType::Deploy,
        "SURRENDER" => DamageType::Surrender,
        "HACK" => DamageType::Hack,
        "KILL_PILOT" | "KILLPILOT" => DamageType::KillPilot,
        "PENALTY" => DamageType::Penalty,
        "FALLING" => DamageType::Falling,
        "MELEE" => DamageType::Melee,
        "DISARM" => DamageType::Disarm,
        "HAZARD_CLEANUP" | "HAZARDCLEANUP" => DamageType::HazardCleanup,
        "TOPPLING" => DamageType::Toppling,
        "INFANTRY_MISSILE" | "INFANTRYMISSILE" => DamageType::InfantryMissile,
        "AURORA_BOMB" | "AURORABOMB" => DamageType::AuroraBomb,
        "LAND_MINE" | "LANDMINE" => DamageType::LandMine,
        "JET_MISSILES" | "JETMISSILES" => DamageType::JetMissiles,
        "STEALTH_JET_MISSILES" | "STEALTHJETMISSILES" => DamageType::StealthJetMissiles,
        "MOLOTOV_COCKTAIL" | "MOLOTOVCOCKTAIL" => DamageType::MolotovCocktail,
        "COMANCHE_VULCAN" | "COMANCHEVULCAN" => DamageType::ComancheVulcan,
        "SUBDUAL_MISSILE" | "SUBDUALMISSILE" => DamageType::SubdualMissile,
        "SUBDUAL_VEHICLE" | "SUBDUALVEHICLE" => DamageType::SubdualVehicle,
        "SUBDUAL_BUILDING" | "SUBDUALBUILDING" => DamageType::SubdualBuilding,
        "SUBDUAL_UNRESISTABLE" | "SUBDUALUNRESISTABLE" => DamageType::SubdualUnresistable,
        "MICROWAVE" => DamageType::Microwave,
        "KILL_GARRISONED" | "KILLGARRISONED" => DamageType::KillGarrisoned,
        "STATUS" => DamageType::Status,
        _ => DamageType::Explosion, // Default
    }
}

fn parse_damage_status_type(s: &str) -> ObjectStatusTypes {
    // C++: ObjectStatusMaskType::parseSingleBitFromINI
    match s.trim().to_ascii_uppercase().as_str() {
        "BURNED" => ObjectStatusTypes::Burned,
        "AFLAME" => ObjectStatusTypes::Aflame,
        "WET" => ObjectStatusTypes::Wet,
        "DESTROYED" => ObjectStatusTypes::Destroyed,
        "CAN_ATTACK" | "CANATTACK" => ObjectStatusTypes::CanAttack,
        "UNDER_CONSTRUCTION" | "UNDERCONSTRUCTION" => ObjectStatusTypes::UnderConstruction,
        "AIRBORNE_TARGET" | "AIRBORNETARGET" => ObjectStatusTypes::AirborneTarget,
        "HIJACKED" => ObjectStatusTypes::Hijacked,
        "IS_FIRING_WEAPON" | "ISFIRINGWEAPON" => ObjectStatusTypes::IsFiringWeapon,
        "BRAKING" => ObjectStatusTypes::Braking,
        "STEALTHED" => ObjectStatusTypes::Stealthed,
        "DETECTED" => ObjectStatusTypes::Detected,
        "SOLD" => ObjectStatusTypes::Sold,
        "UNDERGOING_REPAIR" | "UNDERGOINGREPAIR" => ObjectStatusTypes::UndergoingRepair,
        "IMMOBILE" => ObjectStatusTypes::Immobile,
        "DEPLOYED" => ObjectStatusTypes::Deployed,
        "SUBDUED" => ObjectStatusTypes::Subdued,
        _ => ObjectStatusTypes::None,
    }
}

fn parse_death_type(s: &str) -> DeathType {
    match s.trim().to_ascii_uppercase().as_str() {
        "NONE" => DeathType::None,
        "NORMAL" => DeathType::Normal,
        "EXPLODED" => DeathType::Exploded,
        "BURNED" => DeathType::Burned,
        "CRUSHED" => DeathType::Crushed,
        "POISONED" | "TOXIN" => DeathType::Poisoned,
        "TOPPLED" => DeathType::Toppled,
        "FLOODED" | "SUNK" => DeathType::Flooded,
        "SUICIDED" => DeathType::Suicided,
        "LASERED" => DeathType::Lasered,
        "DETONATED" => DeathType::Detonated,
        "SPLATTED" => DeathType::Splatted,
        "POISONED_BETA" | "POISONEDBETA" | "ANTHRAX_BETA" | "ANTHRAXBETA" | "ANTHRAX" => DeathType::PoisonedBeta,
        "EXTRA2" => DeathType::Extra2,
        "EXTRA3" => DeathType::Extra3,
        "EXTRA4" => DeathType::Extra4,
        "EXTRA5" => DeathType::Extra5,
        "EXTRA6" => DeathType::Extra6,
        "EXTRA7" => DeathType::Extra7,
        "EXTRA8" => DeathType::Extra8,
        "POISONED_GAMMA" | "POISONEDGAMMA" => DeathType::PoisonedGamma,
        _ => DeathType::Normal,
    }
}

fn parse_weapon_reload_type(s: &str) -> WeaponReloadType {
    WeaponReloadType::from_ini(s.trim()).unwrap_or(WeaponReloadType::AutoReload)
}

fn parse_weapon_prefire_type(s: &str) -> WeaponPrefireType {
    WeaponPrefireType::from_ini(s.trim()).unwrap_or(WeaponPrefireType::PrefirePerShot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_template_creation() {
        let template = WeaponTemplate::new("TestWeapon".to_string());
        assert_eq!(template.name, "TestWeapon");
        // C++ default is 0 (0 means unlimited).
        assert_eq!(template.clip_size, 0);
        assert!(!template.is_override());
    }

    #[test]
    fn test_weapon_template_ranges() {
        let mut template = WeaponTemplate::new("RangeTest".to_string());
        template.attack_range = 100.0;
        template.minimum_attack_range = 10.0;

        let bonus = WeaponBonus::new();
        let attack_range = template.get_attack_range(&bonus);
        let min_range = template.get_minimum_attack_range();

        // Should have undersize applied
        assert!(attack_range < 100.0);
        assert!(min_range < 10.0);
        assert!(attack_range > min_range);
    }

    #[test]
    fn test_weapon_template_damage_with_bonus() {
        let mut template = WeaponTemplate::new("DamageTest".to_string());
        template.primary_damage = 50.0;
        template.primary_damage_radius = 25.0;

        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::Damage, 1.5);
        bonus.set_field(WeaponBonusField::Radius, 2.0);

        assert_eq!(template.get_primary_damage(&bonus), 75.0);
        assert_eq!(template.get_primary_damage_radius(&bonus), 50.0);
    }

    #[test]
    fn test_weapon_template_types() {
        let mut template = WeaponTemplate::new("TypeTest".to_string());

        // Contact weapon is determined by range (see `WeaponTemplate::isContactWeapon` in C++).
        template.attack_range = 5.0;
        assert!(template.is_contact_weapon());

        // Non-contact weapon: range large enough to exceed one pathfind cell.
        template.attack_range = 100.0;
        assert!(!template.is_contact_weapon());

        // Test laser weapon
        template.laser_name = "TestLaser".to_string();
        assert!(template.is_laser());
    }

    #[test]
    fn test_weapon_template_timing() {
        let mut template = WeaponTemplate::new("TimingTest".to_string());
        template.min_delay_between_shots = 10;
        template.max_delay_between_shots = 20;
        template.clip_reload_time = 60;
        template.pre_attack_delay = 15;

        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::RateOfFire, 2.0); // Double rate of fire
        bonus.set_field(WeaponBonusField::PreAttack, 0.5); // Half pre-attack delay

        let delay = template.get_delay_between_shots(&bonus);
        let reload = template.get_clip_reload_time(&bonus);
        let pre_attack = template.get_pre_attack_delay(&bonus);

        assert!(delay >= 5 && delay <= 10); // Halved due to 2x rate of fire
        assert_eq!(reload, 30); // Halved due to 2x rate of fire
        assert_eq!(pre_attack, 7); // Halved due to 0.5x pre-attack bonus
    }

    #[test]
    fn test_weapon_template_inheritance() {
        let mut base_template = WeaponTemplate::new("Base".to_string());
        let override_template = WeaponTemplate::new("Override".to_string());

        assert!(!base_template.is_override());

        base_template.set_next_template(override_template);
        assert!(base_template.is_override());
        assert!(base_template.get_next_template().is_some());
    }

    #[test]
    fn test_historic_damage_tracking() {
        let mut template = WeaponTemplate::new("HistoricTest".to_string());
        template.historic_bonus_time = 100;
        let location = Coord3D::new(100.0, 100.0, 0.0);

        template.record_historic_damage(&location, 1000);
        template.record_historic_damage(&location, 1010);
        template.record_historic_damage(&location, 1020);

        // Check that damage was recorded
        let damage_list = template.historic_damage.lock().unwrap();
        assert_eq!(damage_list.len(), 3);
    }

    #[test]
    fn test_effective_scatter_radius_uses_base_for_non_infantry_targets() {
        let mut template = WeaponTemplate::new("ScatterNonInfantry".to_string());
        template.scatter_radius = 35.0;
        template.infantry_inaccuracy_dist = 20.0;

        let scatter = template.effective_scatter_radius(false);
        assert!((scatter - 35.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_effective_scatter_radius_adds_infantry_inaccuracy() {
        let mut template = WeaponTemplate::new("ScatterInfantry".to_string());
        template.scatter_radius = 35.0;
        template.infantry_inaccuracy_dist = 20.0;

        let scatter = template.effective_scatter_radius(true);
        assert!((scatter - 55.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_effective_scatter_radius_ignores_non_positive_infantry_bonus() {
        let mut template = WeaponTemplate::new("ScatterNoBonus".to_string());
        template.scatter_radius = 35.0;
        template.infantry_inaccuracy_dist = 0.0;

        let scatter = template.effective_scatter_radius(true);
        assert!((scatter - 35.0).abs() < f32::EPSILON);
    }
}
