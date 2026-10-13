//! C++ ActiveBody::xfer: selected last DamageInfo and its logic timestamp
//! affect the next damage callback, even when that attacker no longer exists.
use super::{GameLogic, ObjectId, SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BodyDamageHistoryPersist {
    pub(super) object_id: u32,
    pub(super) source_id: Option<u32>,
    pub(super) timestamp: Option<u32>,
    pub(super) previous_health: f32,
    pub(super) damage_state: u8,
}

pub(super) fn capture(world: &GameLogic) -> Vec<BodyDamageHistoryPersist> {
    let mut records: Vec<_> = world
        .host_objects()
        .iter()
        .map(|(id, object)| BodyDamageHistoryPersist {
            object_id: id.0,
            source_id: object.last_damage_source.map(|id| id.0),
            timestamp: object.last_damage_timestamp,
            previous_health: object.previous_health,
            damage_state: object.body_damage_state.ordinal(),
        })
        .collect();
    records.sort_by_key(|entry| entry.object_id);
    records
}

pub(super) fn validate(
    world: &GameLogic,
    records: &[BodyDamageHistoryPersist],
) -> SaveLoadResult<()> {
    let mut owners = std::collections::HashSet::new();
    for entry in records {
        if !owners.insert(entry.object_id)
            || world.host_object(ObjectId(entry.object_id)).is_none()
            || entry.source_id == Some(0)
            || !entry.previous_health.is_finite()
            || entry.previous_health < 0.0
            || entry.damage_state > 3
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved body history owner or source".into(),
            ));
        }
        // A missing current attacker is legitimate: C++ saves the retained ID,
        // and the next hit resolves its existence from the receiving store.
    }
    Ok(())
}

pub(super) fn apply(world: &mut GameLogic, records: Vec<BodyDamageHistoryPersist>) {
    // Earlier OXOB versions did not store this history; do not retain whatever
    // happened to be in a reused receiving Object before that older load.
    for object in world.host_objects_mut().values_mut() {
        object.last_damage_source = None;
        object.last_damage_timestamp = None;
        object.previous_health = object.health.current;
        object.restore_body_observation(None);
    }
    for entry in records {
        let object = world
            .host_object_mut(ObjectId(entry.object_id))
            .expect("validated saved body history owner");
        object.last_damage_source = entry.source_id.map(ObjectId);
        object.last_damage_timestamp = entry.timestamp;
        object.previous_health = entry.previous_health;
        object.restore_body_observation(Some(
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::from_ordinal(
                entry.damage_state,
            ),
        ));
    }
}

#[cfg(test)]
#[path = "body_damage_history_persist/tests.rs"]
mod tests;
