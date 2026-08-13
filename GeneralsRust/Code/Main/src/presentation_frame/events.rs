use super::*;

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
    /// Host research finished this frame (name + player).
    UpgradeComplete {
        name: String,
        player_id: u32,
        team: Team,
        units_affected: u32,
    },
    /// Factory production finished (spawned unit).
    ProductionComplete {
        producer: ObjectId,
        template: String,
        spawned: ObjectId,
    },
    /// Capture / hijack / set_team transfer this frame.
    OwnerChanged {
        id: ObjectId,
        team: Team,
    },
    /// Attack target set this frame (host_attack_log).
    AttackTargeted {
        attacker: ObjectId,
        target: Option<ObjectId>,
    },
    /// Move order destination this frame (host_move_log).
    MoveOrdered {
        unit: ObjectId,
        destination: [f32; 3],
    },
    /// Post-armor HP damage applied this frame (host_damage_log).
    DamageApplied {
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
    },
    /// Absolute HP write this frame (heal / construction finish residual).
    HealApplied {
        target: ObjectId,
        health: f32,
    },
    /// Player supplies/power absolute after host economy mutation.
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
        /// World position residual (ZERO when text-only).
        position: Vec3,
        /// 0=Generic 1=Attack 2=Ally (host RadarKind residual).
        kind: u8,
    },
    /// Wave 533: host EVA pulse (TheEva setShouldPlay residual) for presentation audio.
    EvaAlert {
        name: String,
    },
    /// Combat residual: particle system spawned (host registry id + template).
    ParticleSystemSpawned {
        id: u32,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
    },
    /// C++ FiringTracker looping FireSound start/refresh residual.
    WeaponFireLoopStarted {
        unit: ObjectId,
        sound: String,
    },
    /// C++ FiringTracker stop looping FireSound after FireSoundLoopTime idle.
    WeaponFireLoopStopped {
        unit: ObjectId,
        sound: String,
    },
    /// One concrete accepted WeaponSet discharge.  This is renderer-facing
    /// state only: it is distinct from AI fire intent and identifies the
    /// exact slot/barrel before that cursor advances.
    WeaponDischarged {
        source: ObjectId,
        weapon_slot: u8,
        fired_barrel: u8,
        sequence: u64,
        logic_frame: u32,
    },
}
