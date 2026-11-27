use crate::damage::{CombatDamage, DeathOutcome};
use crate::object::ObjectStatusTypes;
use generals_game_domain::{ObjectId, Team};
use glam::Vec3;

/// Weapon damage submitted to the receiving Body or partition.
#[derive(Debug, Clone)]
pub enum DamageEvent {
    Direct {
        target_id: ObjectId,
        position: Vec3,
        damage: f32,
        damage_type: CombatDamage,
        death_type: DeathOutcome,
        shooter_id: ObjectId,
        /// Fired WeaponTemplate DamageStatusType (C++ DamageInfoInput).
        damage_status_type: Option<ObjectStatusTypes>,
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: CombatDamage,
        death_type: DeathOutcome,
        radius: f32,
        shooter_id: ObjectId,
        /// Outer-ring SecondaryDamage (0 = C++ step uses 0 outside primary).
        secondary_damage: f32,
        secondary_radius: f32,
        /// C++ ShockWave (0 amount = no push).
        shock_wave_amount: f32,
        shock_wave_radius: f32,
        shock_wave_taper_off: f32,
        /// C++ RadiusDamageAffects mask.
        radius_damage_affects: u32,
        /// Shooter team frozen at impact for ally/enemy filter.
        shooter_team: Team,
        /// The C++ radius-damage source may differ from damage attribution:
        /// moving projectile impacts use the projectile, projectileless shots
        /// use the live shooter, and missing shooters bypass source filters.
        radius_source_present: bool,
        radius_source_id: ObjectId,
        radius_source_producer: Option<ObjectId>,
        radius_source_team_id: Option<u32>,
        /// Projectile impact pose is distinct from the launcher's current pose.
        radius_source_pose: Option<(Vec3, (f32, f32))>,
        /// Shooter controlling player frozen at impact (C++ getRelationship).
        shooter_owner_player_id: Option<u32>,
        /// Shooter team instance frozen at impact (PLAYER_SET_OVERRIDE_RELATION_TO_TEAM).
        shooter_team_instance_name: String,
        /// Shooter template name (NOT_SIMILAR `isEquivalentTo` filter).
        shooter_template: String,
        /// C++ `primaryVictim`: intended target skips RadiusDamageAffects.
        primary_victim: Option<ObjectId>,
        /// Effective WeaponTemplate identity resolved by the receiving match.
        radius_weapon_name: String,
        /// Fired WeaponTemplate DamageStatusType (C++ DamageInfoInput).
        damage_status_type: Option<ObjectStatusTypes>,
    },
}
