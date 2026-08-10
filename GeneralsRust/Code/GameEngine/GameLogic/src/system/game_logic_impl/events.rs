/// Game event types for frame-based event tracking
#[derive(Debug, Clone)]
pub enum GameEvent {
    ObjectCreated(ObjectID),
    ObjectDestroyed(ObjectID),
    DamageDealt {
        attacker: ObjectID,
        target: ObjectID,
        amount: f32,
    },
    RadarUpdate {
        player_id: Int,
        position: (f32, f32),
        event_type: RadarEventType,
    },
    BeaconPlaced {
        player_id: Int,
        position: Coord3D,
        text: Option<AsciiString>,
    },
    BeaconRemoved {
        player_id: Int,
        position: Coord3D,
    },
    BeaconTextUpdated {
        player_id: Int,
        position: Coord3D,
        text: AsciiString,
    },
    VictoryConditionMet {
        player_id: Int,
        condition_name: String,
    },
}

/// Game command types for command queue
#[derive(Debug, Clone)]
pub enum GameCommand {
    MoveUnit {
        player_id: Int,
        unit_ids: Vec<ObjectID>,
        target_position: (f32, f32, f32),
    },
    AttackTarget {
        player_id: Int,
        attacker_ids: Vec<ObjectID>,
        target_id: ObjectID,
    },
    BuildStructure {
        player_id: Int,
        builder_id: ObjectID,
        structure_type: String,
        position: (f32, f32),
    },
    UseSpecialPower {
        player_id: Int,
        power_name: String,
        target_position: Option<(f32, f32, f32)>,
    },
}

/// Radar update event
#[derive(Debug, Clone)]
pub struct RadarUpdate {
    pub player_id: Int,
    pub position: (f32, f32),
    pub event_type: RadarEventType,
}

#[derive(Debug, Clone, Copy)]
pub enum RadarEventType {
    UnitCreated,
    UnitDestroyed,
    BaseAttacked,
    EnemyDetected,
    BeaconPlaced,
    BeaconRemoved,
}
