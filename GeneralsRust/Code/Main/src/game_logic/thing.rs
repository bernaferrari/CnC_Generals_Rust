use super::*;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Thing Template - shared configuration data for Things
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThingTemplate {
    pub name: String,
    pub display_name: String,
    pub kind_of: HashSet<KindOf>,
    pub max_health: f32,
    pub armor: f32,
    pub sight_range: f32,
    pub build_cost: Resources,
    pub build_time: f32,
    pub model_name: Option<String>,
    pub texture_name: Option<String>,
    pub special_power_cooldown: f32,
    /// C++ parity: XP awarded to the killer when this object is destroyed.
    /// In C++ this is per-veterancy-level; here we store the Rookie-level
    /// value and scale by veterancy level at kill time.
    pub experience_value: f32,
    /// C++ parity (Object::ExperienceValues): per-template veterancy XP
    /// thresholds [Veteran, Elite, Heroic].  Defaults to [60, 150, 300].
    pub veterancy_xp_thresholds: [f32; 3],
    /// Host primary weapon stats when the template defines combat capability.
    /// Prefer this over ad-hoc `Weapon::default()` injection at create time.
    pub primary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI primary weapon template name (resolved via WeaponStore).
    pub primary_weapon_name: Option<String>,
    /// Host secondary weapon stats (Weapon = SECONDARY Name). Optional; no kind fallback.
    pub secondary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI secondary weapon template name (resolved via WeaponStore).
    pub secondary_weapon_name: Option<String>,
    /// Locomotor.ini SET_NORMAL template name (resolved via Common LocomotorStore).
    /// Fail-closed residual: single primary locomotor only (not multi-set / surface matrix).
    pub locomotor_name: Option<String>,
}

impl ThingTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            kind_of: HashSet::new(),
            max_health: 100.0,
            armor: 0.0,
            sight_range: 150.0,
            build_cost: Resources::default(),
            build_time: 1.0,
            model_name: None,
            texture_name: None,
            special_power_cooldown: 10.0,
            experience_value: 0.0,
            veterancy_xp_thresholds: [60.0, 150.0, 300.0],
            primary_weapon: None,
            primary_weapon_name: None,
            secondary_weapon: None,
            secondary_weapon_name: None,
            locomotor_name: None,
        }
    }

    /// Attach host primary weapon stats (damage/range/reload) to this template.
    pub fn set_primary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.primary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini template name for store lookup at create time.
    pub fn set_primary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.primary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Attach host secondary weapon stats (damage/range/reload) to this template.
    pub fn set_secondary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.secondary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini secondary template name for store lookup at create time.
    /// Fail-closed: "None"/empty does not register a secondary slot.
    pub fn set_secondary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.secondary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Record the Locomotor.ini SET_NORMAL template name for store lookup at create time.
    /// Fail-closed: empty/"None" does not register a locomotor bind.
    pub fn set_locomotor_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.locomotor_name = Some(n.to_string());
        }
        self
    }

    /// Resolve host Movement stats from the Locomotor catalog:
    /// 1) explicit locomotor_name → LocomotorStore (seed/INI)
    /// Fail-closed: no kind-based default — units without a name keep Movement::default().
    pub fn resolve_movement(&self) -> Option<super::locomotor_bootstrap::HostMovementStats> {
        if let Some(name) = self.locomotor_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known locomotors or
            // loads extracted Locomotor.ini when present — see locomotor_bootstrap.rs.
            return super::locomotor_bootstrap::resolve_host_movement(name);
        }
        None
    }

    /// Resolve weapon for a newly created combat unit:
    /// 1) explicit host stats, 2) WeaponStore by primary_weapon_name,
    /// 3) host residual map by template name (`primary_weapon_name_for_unit`),
    /// 4) kind-based default fallback (fail-open last resort for Attackable kinds).
    pub fn resolve_primary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.primary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.primary_weapon_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known weapons or
            // loads extracted Weapon.ini when present — see weapon_bootstrap.rs.
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(name) {
                return Some(w);
            }
        }
        // Host residual map: templates often omit primary_weapon_name (units.rs /
        // setup_templates gaps) but have a known retail weapon for the unit name.
        // Prefer store residual over kind-based Weapon::default().
        if let Some(wname) = super::weapon_bootstrap::primary_weapon_name_for_unit(&self.name) {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(wname) {
                return Some(w);
            }
        }
        if self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
            || self.is_kind_of(KindOf::Attackable)
        {
            // Last-resort host combat stats when no template/store weapon is usable.
            return Some(Weapon::default());
        }
        None
    }

    /// Resolve secondary weapon for a newly created combat unit.
    /// Fail-closed (not full WeaponSet):
    /// 1) explicit host stats, 2) WeaponStore by secondary_weapon_name,
    /// 3) host residual map by template name (`secondary_weapon_name_for_unit`).
    /// No kind-based `Weapon::default()` fallback — units without SECONDARY stay unarmed there.
    pub fn resolve_secondary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.secondary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.secondary_weapon_name.as_deref() {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(name) {
                return Some(w);
            }
        }
        // Host residual map: secondary slot by unit template name when not set.
        if let Some(wname) = super::weapon_bootstrap::secondary_weapon_name_for_unit(&self.name) {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(wname) {
                return Some(w);
            }
        }
        None
    }

    /// Convert a gamelogic WeaponStore template into Main host Weapon stats.
    /// Returns None if store is missing or stats are unusable (0 dmg/range).
    pub fn weapon_from_store(name: &str) -> Option<Weapon> {
        use gamelogic::weapon::{with_weapon_store, WeaponAntiMask};
        const FPS: f32 = 30.0;
        let wt = with_weapon_store(|store| store.find_weapon_template(name).cloned()).ok()??;
        if wt.primary_damage <= 0.0 || wt.attack_range <= 0.0 {
            return None;
        }
        let between_frames = wt.min_delay_between_shots.max(0) as f32;
        let clip_frames = wt.max_delay_between_shots.max(0) as f32;
        let delay_frames = if wt.clip_size > 0 {
            // Within-clip cadence residual (C++ DelayBetweenShots).
            if between_frames > 0.0 {
                between_frames
            } else {
                clip_frames
            }
        } else {
            between_frames.max(clip_frames)
        };
        let reload_time = if delay_frames > 0.0 {
            delay_frames / FPS
        } else {
            1.0
        };
        let pre_attack_delay = (wt.pre_attack_delay.max(0) as f32) / FPS;
        let projectile_speed = if wt.weapon_speed >= 999_999.0 {
            0.0
        } else {
            wt.weapon_speed
        };
        Some(Weapon {
            damage: wt.primary_damage,
            range: wt.attack_range,
            min_range: wt.minimum_attack_range.max(0.0),
            reload_time,
            last_fire_time: 0.0,
            ammo: if wt.clip_size > 0 {
                Some(wt.clip_size as u32)
            } else {
                None
            },
            clip_size: wt.clip_size.max(0) as u32,
            // Clip reload residual: store often encodes clip reload as max delay.
            clip_reload_time: if wt.clip_size > 0 {
                (wt.max_delay_between_shots.max(0) as f32) / 30.0
            } else {
                0.0
            },
            can_target_air: wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE)
                || wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_INFANTRY),
            can_target_ground: wt.anti_mask.contains(WeaponAntiMask::GROUND)
                || !wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE),
            projectile_speed,
            pre_attack_delay,
            splash_radius: wt.primary_damage_radius.max(0.0),
        })
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of.contains(&kind)
    }

    pub fn add_kind_of(&mut self, kind: KindOf) -> &mut Self {
        self.kind_of.insert(kind);
        self
    }

    pub fn set_health(&mut self, health: f32) -> &mut Self {
        self.max_health = health;
        self
    }

    pub fn set_cost(&mut self, supplies: u32, power: i32) -> &mut Self {
        self.build_cost = Resources { supplies, power };
        self
    }

    pub fn set_model(&mut self, model: &str) -> &mut Self {
        self.model_name = Some(model.to_string());
        self
    }

    /// Get the model name for this template, or fall back to template name
    pub fn get_model_name(&self) -> &str {
        self.model_name.as_deref().unwrap_or(&self.name)
    }

    /// Get the W3D model filename (with .w3d extension if needed)
    pub fn get_w3d_filename(&self) -> String {
        let model_name = self.get_model_name();
        if model_name.to_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{}.w3d", model_name)
        }
    }
}

#[cfg(test)]
mod weapon_resolve_tests {
    use super::*;

    #[test]
    fn explicit_primary_weapon_beats_store_and_default() {
        let mut t = ThingTemplate::new("Armed");
        t.add_kind_of(KindOf::Infantry);
        t.set_primary_weapon(Weapon {
            damage: 40.0,
            range: 80.0,
            reload_time: 0.5,
            ..Weapon::default()
        });
        t.set_primary_weapon_name("DoesNotExistInStoreHopefully");
        let w = t.resolve_primary_weapon().expect("weapon");
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 80.0).abs() < 0.01);
    }

    #[test]
    fn infantry_without_weapon_gets_kind_fallback() {
        let mut t = ThingTemplate::new("BareInfantry");
        t.add_kind_of(KindOf::Infantry);
        let w = t.resolve_primary_weapon().expect("fallback");
        assert!((w.damage - Weapon::default().damage).abs() < 0.01);
    }

    #[test]
    fn structure_without_weapon_stays_unarmed() {
        let mut t = ThingTemplate::new("BareStructure");
        t.add_kind_of(KindOf::Structure);
        assert!(t.resolve_primary_weapon().is_none());
    }

    #[test]
    fn primary_weapon_name_resolves_non_default_store_stats() {
        // Prove store bind path for USA_Ranger / GoldenRanger weapon name.
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_name(super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON);
        let w = t.resolve_primary_weapon().expect("store-bound weapon");
        assert!(
            (w.damage - Weapon::default().damage).abs() > 0.01,
            "store path must not yield host default damage; got {}",
            w.damage
        );
        assert!((w.damage - 5.0).abs() < 0.01);
        assert!((w.range - 100.0).abs() < 0.01);
    }

    #[test]
    fn secondary_weapon_name_resolves_non_default_store_stats() {
        // Prove SECONDARY store bind path (Ranger flashbang residual).
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_secondary_weapon_name(super::super::weapon_bootstrap::RANGER_SECONDARY_WEAPON);
        let w = t.resolve_secondary_weapon().expect("store-bound secondary");
        assert!(
            (w.damage - Weapon::default().damage).abs() > 0.01,
            "secondary store path must not yield host default damage; got {}",
            w.damage
        );
        // Retail RangerFlashBangGrenadeWeapon PrimaryDamage 35, AttackRange 175.
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
    }

    #[test]
    fn secondary_without_name_stays_none_even_for_infantry() {
        // Fail-closed: no kind-based default for secondary slots.
        let mut t = ThingTemplate::new("BareInfantry");
        t.add_kind_of(KindOf::Infantry);
        assert!(t.resolve_secondary_weapon().is_none());
    }

    #[test]
    fn unit_name_residual_map_binds_without_explicit_weapon_name() {
        // units.rs / setup_templates often omit primary_weapon_name; residual map
        // must still prefer retail store stats over kind-based Weapon::default.
        let mut technical = ThingTemplate::new("GLA_Technical");
        technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        let tw = technical
            .resolve_primary_weapon()
            .expect("technical residual weapon");
        assert!(
            (tw.damage - Weapon::default().damage).abs() > 0.01,
            "GLA_Technical must not fall through to Weapon::default (got dmg={})",
            tw.damage
        );
        // Retail TechnicalMachineGunWeapon PrimaryDamage 10.
        assert!((tw.damage - 10.0).abs() < 0.01);
        assert!((tw.range - 150.0).abs() < 0.01);

        let mut battle = ThingTemplate::new("China_BattleTank");
        battle
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        let bw = battle
            .resolve_primary_weapon()
            .expect("battlemaster residual weapon");
        assert!(
            (bw.damage - Weapon::default().damage).abs() > 0.01,
            "China_BattleTank must not fall through to Weapon::default (got dmg={})",
            bw.damage
        );
        // Retail BattleMasterTankGun PrimaryDamage 60.
        assert!((bw.damage - 60.0).abs() < 0.01);
        assert!((bw.range - 150.0).abs() < 0.01);
    }

    #[test]
    fn secondary_unit_name_residual_map_binds_ranger_flashbang() {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable);
        // No secondary_weapon_name set — residual map by template name.
        let w = t
            .resolve_secondary_weapon()
            .expect("ranger residual secondary");
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
    }

    #[test]
    fn explicit_secondary_weapon_beats_store() {
        let mut t = ThingTemplate::new("Armed");
        t.set_secondary_weapon(Weapon {
            damage: 99.0,
            range: 50.0,
            reload_time: 1.0,
            ..Weapon::default()
        });
        t.set_secondary_weapon_name("DoesNotExistInStoreHopefully");
        let w = t.resolve_secondary_weapon().expect("weapon");
        assert!((w.damage - 99.0).abs() < 0.01);
        assert!((w.range - 50.0).abs() < 0.01);
    }
}

/// Base Thing class - common functionality for all game entities
#[derive(Debug, Serialize, Deserialize)]
pub struct Thing {
    pub template: ThingTemplate,
    pub geometry: GeometryInfo,
    pub transform: Mat4,

    // Cached values for performance
    cached_position: Vec3,
    cached_angle: f32,
    cached_dir_vector: Vec3,
    cache_valid: bool,
}

impl Thing {
    pub fn new(template: ThingTemplate) -> Self {
        let mut thing = Self {
            template,
            geometry: GeometryInfo::default(),
            transform: Mat4::IDENTITY,
            cached_position: Vec3::ZERO,
            cached_angle: 0.0,
            cached_dir_vector: Vec3::X,
            cache_valid: false,
        };
        thing.update_cache();
        thing
    }

    pub fn get_template(&self) -> &ThingTemplate {
        &self.template
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.template.is_kind_of(kind)
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.geometry.position = position;
        self.transform =
            Mat4::from_translation(position) * Mat4::from_rotation_y(self.cached_angle);
        self.update_cache();
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.cached_angle = angle;
        self.transform =
            Mat4::from_translation(self.cached_position) * Mat4::from_rotation_y(angle);
        self.update_cache();
    }

    pub fn get_position(&self) -> Vec3 {
        self.cached_position
    }

    pub fn get_orientation(&self) -> f32 {
        self.cached_angle
    }

    pub fn get_direction_vector(&self) -> Vec3 {
        self.cached_dir_vector
    }

    pub fn set_transform_matrix(&mut self, transform: Mat4) {
        self.transform = transform;
        self.update_cache();
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.transform
    }

    fn update_cache(&mut self) {
        // Extract position from transform matrix
        let translation = self.transform.w_axis.truncate();
        self.cached_position = translation;

        // Extract rotation angle (assuming rotation around Y axis)
        let forward = self.transform.z_axis.truncate();
        self.cached_angle = (-forward.z).atan2(forward.x);

        // Calculate direction vector
        self.cached_dir_vector = Vec3::new(self.cached_angle.cos(), 0.0, -self.cached_angle.sin());

        // Update geometry position
        self.geometry.position = self.cached_position;
        self.geometry.rotation = self.cached_angle;

        self.cache_valid = true;
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        (self.transform * point.extend(1.0)).truncate()
    }

    pub fn get_distance_to(&self, other: &Thing) -> f32 {
        self.cached_position.distance(other.cached_position)
    }

    pub fn get_distance_to_position(&self, position: Vec3) -> f32 {
        self.cached_position.distance(position)
    }

    pub fn is_within_range(&self, other: &Thing, range: f32) -> bool {
        self.get_distance_to(other) <= range
    }

    pub fn get_bounds(&self) -> (Vec3, Vec3) {
        let half_size = Vec3::splat(self.geometry.radius);
        (
            self.cached_position - half_size,
            self.cached_position + half_size,
        )
    }

    pub fn intersects_bounds(&self, other: &Thing) -> bool {
        let (min_a, max_a) = self.get_bounds();
        let (min_b, max_b) = other.get_bounds();

        max_a.x >= min_b.x
            && min_a.x <= max_b.x
            && max_a.y >= min_b.y
            && min_a.y <= max_b.y
            && max_a.z >= min_b.z
            && min_a.z <= max_b.z
    }
}

impl Clone for Thing {
    fn clone(&self) -> Self {
        Self {
            template: self.template.clone(),
            geometry: self.geometry.clone(),
            transform: self.transform,
            cached_position: self.cached_position,
            cached_angle: self.cached_angle,
            cached_dir_vector: self.cached_dir_vector,
            cache_valid: self.cache_valid,
        }
    }
}
