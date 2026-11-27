//! Dyn-Xfer helpers and game_logic primitive XferData impls.

use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Helper functions for Vec/HashMap/Option xfer (dyn Xfer safe)
// ---------------------------------------------------------------------------

pub(super) fn xfer_vec_default<T: Clone + XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Vec<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut item = default.clone();
            item.xfer(xfer)?;
            data.push(item);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_option<T: XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Option<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut is_some = data.is_some();
    xfer.xfer_bool(&mut is_some)?;
    if is_some {
        if data.is_none() {
            *data = Some(default);
        }
        if let Some(val) = data.as_mut() {
            val.xfer(xfer)?;
        }
    } else {
        *data = None;
    }
    Ok(())
}

pub(super) fn xfer_hashmap_default<K, V>(
    xfer: &mut dyn Xfer,
    data: &mut HashMap<K, V>,
    key_default: K,
    val_default: V,
) -> SaveLoadResult<()>
where
    K: Clone + std::hash::Hash + Eq + XferData,
    V: Clone + XferData,
{
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut k = key_default.clone();
            let mut v = val_default.clone();
            k.xfer(xfer)?;
            v.xfer(xfer)?;
            data.insert(k, v);
        }
    } else {
        for (k, v) in data.iter_mut() {
            let mut kc = k.clone();
            kc.xfer(xfer)?;
            v.xfer(xfer)?;
        }
    }
    Ok(())
}

/// AI continuation JSON is not an original C++ ASCII name: it may exceed the
/// one-byte length limit of `xfer_string`. Keep it bounded and detached until
/// the complete UTF-8 payload has been read.
pub(super) const MAX_AI_CONTINUATION_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn xfer_ai_continuation_string(
    xfer: &mut dyn Xfer,
    value: &mut String,
) -> SaveLoadResult<usize> {
    let mut bytes = if xfer.get_mode() == XferMode::Load {
        Vec::new()
    } else {
        value.as_bytes().to_vec()
    };
    if bytes.len() > MAX_AI_CONTINUATION_BYTES {
        return Err(SaveLoadError::Corrupted(
            "AI continuation exceeds size limit".into(),
        ));
    }
    let mut len = bytes.len() as u32;
    xfer.xfer_u32(&mut len)?;
    let len = len as usize;
    if len > MAX_AI_CONTINUATION_BYTES {
        return Err(SaveLoadError::Corrupted(
            "AI continuation exceeds size limit".into(),
        ));
    }
    if xfer.get_mode() == XferMode::Load {
        // Do not allocate the declared size until the reader has supplied it.
        while bytes.len() < len {
            let mut block = vec![0; (len - bytes.len()).min(4096)];
            xfer.xfer_raw(&mut block)?;
            bytes.extend_from_slice(&block);
        }
        *value = String::from_utf8(bytes).map_err(|error| {
            SaveLoadError::Corrupted(format!("invalid AI continuation UTF-8: {error}"))
        })?;
    } else {
        xfer.xfer_raw(&mut bytes)?;
    }
    Ok(len)
}

pub(super) fn xfer_serde_blob<T>(xfer: &mut dyn Xfer, value: &mut T) -> SaveLoadResult<()>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    let mut bytes = if xfer.get_mode() == XferMode::Load {
        Vec::new()
    } else {
        bincode_legacy::serialize(value).map_err(|e| SaveLoadError::Serialization(e.to_string()))?
    };
    let mut len = bytes.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        bytes.resize(len as usize, 0);
    }
    if !bytes.is_empty() {
        xfer.xfer_raw(&mut bytes)?;
    }
    if xfer.get_mode() == XferMode::Load {
        *value = if bytes.is_empty() {
            T::default()
        } else {
            bincode_legacy::deserialize(&bytes)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?
        };
    }
    Ok(())
}

pub(super) fn xfer_vec_f32(xfer: &mut dyn Xfer, data: &mut Vec<f32>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = 0.0f32;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_bool(xfer: &mut dyn Xfer, data: &mut Vec<bool>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = false;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_u8(xfer: &mut dyn Xfer, data: &mut Vec<u8>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        data.reserve(len as usize);
        for _ in 0..len {
            let mut val = 0u8;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_vec3(xfer: &mut dyn Xfer, data: &mut Vec<glam::Vec3>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = glam::Vec3::ZERO;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Default constructors for complex snapshot types (used during load)
// ---------------------------------------------------------------------------

pub(super) fn default_object_snapshot() -> ObjectSnapshot {
    ObjectSnapshot {
        id: ObjectId(0),
        template_name: String::new(),
        team: Team::Neutral,
        player_id: 0,
        geometry: GeometryInfo::default(),
        status: ObjectStatusSnapshot::default(),
        health: Health {
            current: 0.0,
            maximum: 0.0,
        },
        movement: Movement::default(),
        experience: Experience::default(),
        weapons: Vec::new(),
        contained_objects: Vec::new(),
        container_object: None,
        modules: HashMap::new(),
        object_type: ObjectTypeSnapshot::Unit(UnitSnapshot {
            unit_type: String::new(),
            formation_position: None,
            formation_id: None,
            group_id: None,
            waypoints: Vec::new(),
        }),
        hacker_disable_channel: None,
        weapon_barrel_states: default_weapon_barrel_state_snapshots(),
        last_weapon_discharge_sequence: 0,
        last_weapon_discharge_slot: 0,
        last_weapon_discharge_barrel: 0,
        last_weapon_discharge_frame: 0,
        collector_runtime: None,
        weapon_suspend_fx_frames: Vec::new(),
        temporary_weapon_runtime: None,
        weapon_bonus_frenzy: false,
        weapon_bonus_frenzy_level: 0,
        weapon_bonus_frenzy_until_frame: 0,
        weapon_frame_clocks: [(0, false, 0); 3],
        weapon_bonus_target_faerie_fire: false,
        weapon_bonus_garrisoned: false,
        weapon_frame_statuses: Default::default(),
    }
}

pub(super) fn default_player_snapshot() -> PlayerSnapshot {
    PlayerSnapshot {
        id: 0,
        name: String::new(),
        team: Team::Neutral,
        is_human: false,
        is_active: false,
        resources: Resources::default(),
        population: PopulationInfo {
            current: 0,
            maximum: 0,
        },
        tech_tree: TechTreeSnapshot {
            unlocked_units: Vec::new(),
            unlocked_buildings: Vec::new(),
            unlocked_upgrades: Vec::new(),
            research_progress: HashMap::new(),
        },
        upgrades: Vec::new(),
        build_queue: Vec::new(),
        research_queue: Vec::new(),
        statistics: PlayerStatisticsSnapshot {
            units_built: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_lost: 0,
            damage_dealt: 0.0,
            damage_received: 0.0,
            resources_gathered: 0,
            experience_gained: 0.0,
        },
    }
}

pub(super) fn default_ai_strategic_state() -> AIStrategicStateSnapshot {
    AIStrategicStateSnapshot {
        current_phase: String::new(),
        objectives: Vec::new(),
        threat_assessment: ThreatAssessmentSnapshot {
            enemy_strengths: HashMap::new(),
            vulnerable_areas: Vec::new(),
            threat_level: 0.0,
        },
    }
}

pub(super) fn default_ai_tactical_state() -> AITacticalStateSnapshot {
    AITacticalStateSnapshot {
        unit_groups: Vec::new(),
        active_attacks: Vec::new(),
        defensive_positions: Vec::new(),
    }
}

pub(super) fn default_ai_economic_state() -> AIEconomicStateSnapshot {
    AIEconomicStateSnapshot {
        build_priorities: Vec::new(),
        economic_focus: String::new(),
        resource_allocation: ResourceAllocation {
            military_percentage: 0.0,
            economic_percentage: 0.0,
            defensive_percentage: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for game_logic types
// ---------------------------------------------------------------------------

impl XferData for GeometryInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("GeometryInfo")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Rotation")?;
        xfer.xfer_f32(&mut self.rotation)?;
        xfer.xfer_marker_label("BoundsMin")?;
        self.bounds_min.xfer(xfer)?;
        xfer.xfer_marker_label("BoundsMax")?;
        self.bounds_max.xfer(xfer)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        Ok(())
    }
}

impl XferData for Health {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Health")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Maximum")?;
        xfer.xfer_f32(&mut self.maximum)?;
        Ok(())
    }
}

impl XferData for Resources {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Resources")?;
        xfer.xfer_marker_label("Supplies")?;
        xfer.xfer_u32(&mut self.supplies)?;
        xfer.xfer_marker_label("Power")?;
        xfer.xfer_i32(&mut self.power)?;
        Ok(())
    }
}

impl XferData for Movement {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        if xfer.get_mode() == XferMode::Load {
            let mut decoded = Movement::default();
            xfer_movement(&mut decoded, xfer)?;
            *self = decoded;
            Ok(())
        } else {
            validate_movement_path(self)?;
            xfer_movement(self, xfer)
        }
    }
}

fn xfer_movement(movement: &mut Movement, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
    xfer.xfer_marker_label("Movement")?;
    xfer.xfer_marker_label("TargetPosition")?;
    xfer_option(xfer, &mut movement.target_position, glam::Vec3::ZERO)?;
    xfer.xfer_marker_label("Velocity")?;
    movement.velocity.xfer(xfer)?;
    xfer.xfer_marker_label("MaxSpeed")?;
    xfer.xfer_f32(&mut movement.max_speed)?;
    xfer.xfer_marker_label("Acceleration")?;
    xfer.xfer_f32(&mut movement.acceleration)?;
    xfer.xfer_marker_label("TurnRate")?;
    xfer.xfer_f32(&mut movement.turn_rate)?;
    xfer.xfer_marker_label("MaxSpeedDamaged")?;
    xfer.xfer_f32(&mut movement.max_speed_damaged)?;
    xfer.xfer_marker_label("AccelerationDamaged")?;
    xfer.xfer_f32(&mut movement.acceleration_damaged)?;
    xfer.xfer_marker_label("TurnRateDamaged")?;
    xfer.xfer_f32(&mut movement.turn_rate_damaged)?;
    xfer.xfer_marker_label("Path")?;
    // Like Path::xfer, retain the raw list and its optimized links rather
    // than saving only the visible optimized waypoints. Unknown layers are
    // represented explicitly; they are not equivalent to ground.
    let mut version = 1;
    xfer.xfer_version(&mut version, 1)?;
    if version != 1 {
        return Err(SaveLoadError::VersionMismatch { expected: 1, actual: version as u32 });
    }
    let loading = xfer.get_mode() == XferMode::Load;
    let mut count = u32::try_from(movement.path.nodes().len())
        .map_err(|_| SaveLoadError::Corrupted("path has too many nodes".into()))?;
    xfer.xfer_u32(&mut count)?;
    let mut nodes = Vec::new();
    for index in (0..count as usize).rev() {
        let mut node = if loading {
            PathRouteNode { position: Vec3::ZERO, layer: None, can_optimize: false, next_optimized: None }
        } else {
            movement.path.nodes()[index].clone()
        };
        node.position.xfer(xfer)?;
        let mut has_layer = node.layer.is_some();
        xfer.xfer_bool(&mut has_layer)?;
        if has_layer {
            let mut layer = node.layer.unwrap_or(0);
            xfer.xfer_u8(&mut layer)?;
            node.layer = Some(layer);
        } else {
            node.layer = None;
        }
        xfer.xfer_bool(&mut node.can_optimize)?;
        let mut has_next = node.next_optimized.is_some();
        xfer.xfer_bool(&mut has_next)?;
        if has_next {
            let mut next = u32::try_from(node.next_optimized.unwrap_or(0))
                .map_err(|_| SaveLoadError::Corrupted("path link is too large".into()))?;
            xfer.xfer_u32(&mut next)?;
            if next as usize <= index || next >= count {
                return Err(SaveLoadError::Corrupted(format!(
                    "invalid optimized path link {index} -> {next} ({count} raw nodes)"
                )));
            }
            node.next_optimized = Some(next as usize);
        } else {
            node.next_optimized = None;
        }
        if loading {
            nodes.push(node);
        }
    }
    if loading {
        nodes.reverse();
        let path = PathRoute::from_nodes(nodes, movement.path.blocked_by_ally)
            .ok_or_else(|| SaveLoadError::Corrupted("invalid optimized path links".into()))?;
        movement.path = path;
    }
    xfer.xfer_bool(&mut movement.path.blocked_by_ally)?;
    xfer.xfer_marker_label("PreviousPathNode")?;
    let encode_index = |index: Option<usize>| -> SaveLoadResult<u32> {
        match index {
            None => Ok(u32::MAX),
            Some(index) => u32::try_from(index).ok().filter(|&index| index != u32::MAX)
                .ok_or_else(|| SaveLoadError::Corrupted("path cursor index is too large".into())),
        }
    };
    let mut previous = encode_index(movement.path_cursor.previous())?;
    let mut current = encode_index(movement.path_cursor.current())?;
    xfer.xfer_u32(&mut previous)?;
    xfer.xfer_marker_label("CurrentPathNode")?;
    xfer.xfer_u32(&mut current)?;
    let previous = (previous != u32::MAX).then_some(previous as usize);
    let current = (current != u32::MAX).then_some(current as usize);
    movement.path_cursor = movement.path.cursor_from_raw(previous, current)
        .ok_or_else(|| SaveLoadError::Corrupted("invalid movement path cursor".into()))?;
    Ok(())
}

impl XferData for VeterancyLevel {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut disc: u32 = match self {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        *self = match disc {
            0 => VeterancyLevel::Rookie,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid VeterancyLevel: {disc}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for Experience {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Experience")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Level")?;
        self.level.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for Weapon {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Weapon")?;
        xfer.xfer_marker_label("Damage")?;
        xfer.xfer_f32(&mut self.damage)?;
        xfer.xfer_marker_label("Range")?;
        xfer.xfer_f32(&mut self.range)?;
        xfer.xfer_marker_label("MinRange")?;
        xfer.xfer_f32(&mut self.min_range)?;
        xfer.xfer_marker_label("ReloadTime")?;
        xfer.xfer_f32(&mut self.reload_time)?;
        xfer.xfer_marker_label("LastFireTime")?;
        xfer.xfer_f32(&mut self.last_fire_time)?;
        xfer.xfer_marker_label("LastFireFrame")?;
        xfer.xfer_u32(&mut self.last_fire_frame)?;
        xfer.xfer_marker_label("Ammo")?;
        xfer_option(xfer, &mut self.ammo, 0u32)?;
        xfer.xfer_marker_label("CanTargetAir")?;
        xfer.xfer_bool(&mut self.can_target_air)?;
        xfer.xfer_marker_label("CanTargetGround")?;
        xfer.xfer_bool(&mut self.can_target_ground)?;
        xfer.xfer_marker_label("ProjectileSpeed")?;
        xfer.xfer_f32(&mut self.projectile_speed)?;
        xfer.xfer_marker_label("PreAttackDelay")?;
        xfer.xfer_f32(&mut self.pre_attack_delay)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for snapshot types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_load::{XferLoad, XferSave};
    use std::io::Cursor;

    #[test]
    fn movement_xfer_keeps_the_live_locomotor_speed() {
        let mut source = Movement::default();
        source.max_speed = 35.0;
        source.acceleration = 12.0;
        source.turn_rate = 1.25;
        source.max_speed_damaged = 18.0;
        source.acceleration_damaged = 6.0;
        source.turn_rate_damaged = 0.4;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = XferSave::new(&mut bytes);
            source.xfer(&mut writer).expect("write movement");
        }
        bytes.set_position(0);
        let mut loaded = Movement::default();
        {
            let mut reader = XferLoad::new(bytes);
            loaded.xfer(&mut reader).expect("read movement");
        }
        assert!((loaded.max_speed - 35.0).abs() < 0.001);
        assert!((loaded.acceleration - 12.0).abs() < 0.001);
        assert!((loaded.turn_rate - 1.25).abs() < 0.001);
        assert!((loaded.max_speed_damaged - 18.0).abs() < 0.001);
        assert!((loaded.acceleration_damaged - 6.0).abs() < 0.001);
        assert!((loaded.turn_rate_damaged - 0.4).abs() < 0.001);
    }

    #[test]
    fn movement_round_trip_preserves_exhausted_cursor_across_appended_nodes() {
        let mut source = Movement::default();
        source.set_path_coords(vec![Vec3::ZERO, Vec3::X]);
        source.path_cursor = source.path.cursor_at_ordinal(1);
        source.path_cursor.advance(&source.path);
        source.path.push(Vec3::Y);
        assert_eq!(source.path_cursor.previous(), Some(1));
        assert_eq!(source.path_cursor.current(), None);

        let binary = bincode_legacy::serialize(&source).unwrap();
        let binary_loaded: Movement = bincode_legacy::deserialize(&binary).unwrap();
        assert_eq!(binary_loaded.path_cursor, source.path_cursor);
        assert_eq!(binary_loaded.path, source.path);

        let mut bytes = Cursor::new(Vec::new());
        source.xfer(&mut XferSave::new(&mut bytes)).unwrap();
        let mut loaded = Movement::default();
        loaded.xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner()))).unwrap();
        assert_eq!(loaded.path_cursor, source.path_cursor);
        assert_eq!(loaded.path, source.path);
        loaded.path_cursor.advance(&loaded.path);
        assert_eq!(loaded.path_cursor.previous(), Some(1));
        assert_eq!(loaded.path_cursor.current(), Some(2));
    }

    #[test]
    fn movement_xfer_rejects_invalid_cursor_without_mutating_receiver() {
        let mut source = Movement::default();
        source.set_path_coords(vec![Vec3::ZERO, Vec3::X, Vec3::Y]);
        source.path_cursor = source.path.cursor_at_ordinal(1);
        let mut bytes = Cursor::new(Vec::new());
        source.xfer(&mut XferSave::new(&mut bytes)).unwrap();
        let bytes = bytes.into_inner();
        for (previous, current) in [(0u32, 3u32), (0, 2), (2, 1)] {
            let mut malformed = bytes.clone();
            let cursor_offset = malformed.len() - 8;
            malformed[cursor_offset..cursor_offset + 4].copy_from_slice(&previous.to_le_bytes());
            malformed[cursor_offset + 4..].copy_from_slice(&current.to_le_bytes());
            let mut receiver = Movement::default();
            receiver.set_path_coords(vec![Vec3::Z]);
            receiver.target_position = Some(Vec3::Z);
            receiver.max_speed = 123.0;
            let before = receiver.clone();
            assert!(receiver.xfer(&mut XferLoad::new(Cursor::new(malformed))).is_err());
            assert_eq!(receiver.path, before.path);
            assert_eq!(receiver.path_cursor, before.path_cursor);
            assert_eq!(receiver.target_position, before.target_position);
            assert_eq!(receiver.max_speed, before.max_speed);
        }
    }
}
