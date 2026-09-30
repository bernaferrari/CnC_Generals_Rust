use crate::weapon_visual_dispatch::FrozenWeaponVisualDispatchPlan;
use crate::{CombatParticleKind, ObjectId, Team};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Ordered gameplay event for audio/FX/UI (presentation side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PresentationEvent {
    ObjectDestroyed {
        id: ObjectId,
        team: Team,
    },
    ConstructionComplete {
        id: ObjectId,
        template: String,
    },
    UpgradeComplete {
        name: String,
        player_id: u32,
        team: Team,
        units_affected: u32,
    },
    ProductionComplete {
        producer: ObjectId,
        template: String,
        spawned: ObjectId,
    },
    OwnerChanged {
        id: ObjectId,
        team: Team,
    },
    AttackTargeted {
        attacker: ObjectId,
        target: Option<ObjectId>,
    },
    MoveOrdered {
        unit: ObjectId,
        destination: [f32; 3],
    },
    DamageApplied {
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
    },
    HealApplied {
        target: ObjectId,
        health: f32,
    },
    EconomyChanged {
        player_id: u32,
        supplies: u32,
        power_available: i32,
    },
    Victory {
        winner_player: Option<u32>,
    },
    RadarMessage {
        team: Team,
        text: String,
        position: Vec3,
        kind: u8,
    },
    EvaAlert {
        name: String,
    },
    ParticleSystemSpawned {
        id: u32,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
    },
    WeaponFireLoopStarted {
        unit: ObjectId,
        sound: String,
    },
    WeaponFireLoopStopped {
        unit: ObjectId,
        sound: String,
    },
    WeaponDischarged {
        source: ObjectId,
        weapon_slot: u8,
        fired_barrel: u8,
        sequence: u64,
        logic_frame: u32,
        visual_plan: Option<FrozenWeaponVisualDispatchPlan>,
    },
}
