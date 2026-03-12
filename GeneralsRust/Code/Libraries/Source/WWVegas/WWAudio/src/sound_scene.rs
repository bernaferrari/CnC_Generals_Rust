//! Scene graph for audible content, mirroring WWAudio's `SoundSceneClass` responsibilities.

use crate::{
    audible_sound::AudibleSound,
    listener::Listener3D,
    logical_listener::LogicalListener,
    logical_sound::LogicalSound,
    math::{Matrix3D, Vector3},
    sound3d::Sound3D,
    sound_pseudo3d::SoundPseudo3D,
    sound_scene_obj::SoundObjectId,
};
use std::collections::{HashMap, HashSet};

const MAX_LOGICAL_LISTENER_UPDATES_PER_FRAME: usize = 4;

#[derive(Debug, Clone)]
pub struct LogicalTrigger {
    pub listener_id: SoundObjectId,
    pub sound_id: SoundObjectId,
    pub type_mask: u32,
    pub is_single_shot: bool,
}

#[derive(Clone)]
struct LogicalSpatialIndex {
    cell_size: f32,
    inverse_cell_size: f32,
    cells: HashMap<(i32, i32, i32), Vec<usize>>,
    max_radius: f32,
}

impl LogicalSpatialIndex {
    fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            inverse_cell_size: if cell_size > 0.0 {
                1.0 / cell_size
            } else {
                1.0
            },
            cells: HashMap::new(),
            max_radius: 0.0,
        }
    }

    fn rebuild(&mut self, sounds: &[LogicalSound]) {
        self.cells.clear();
        self.max_radius = 0.0;
        for (index, sound) in sounds.iter().enumerate() {
            let position = sound.position();
            let key = self.cell_key(position);
            self.cells.entry(key).or_default().push(index);
            self.max_radius = self.max_radius.max(sound.dropoff_radius());
        }
    }

    fn cell_key(&self, position: Vector3) -> (i32, i32, i32) {
        let inv = self.inverse_cell_size;
        (
            (position.x * inv).floor() as i32,
            (position.y * inv).floor() as i32,
            (position.z * inv).floor() as i32,
        )
    }

    fn candidates(&self, center: Vector3, radius: f32) -> Vec<usize> {
        if self.cells.is_empty() {
            return Vec::new();
        }

        let search_radius = radius.max(self.cell_size);
        let inv = self.inverse_cell_size;
        let min_x = ((center.x - search_radius) * inv).floor() as i32;
        let max_x = ((center.x + search_radius) * inv).floor() as i32;
        let min_y = ((center.y - search_radius) * inv).floor() as i32;
        let max_y = ((center.y + search_radius) * inv).floor() as i32;
        let min_z = ((center.z - search_radius) * inv).floor() as i32;
        let max_z = ((center.z + search_radius) * inv).floor() as i32;

        let mut candidates = Vec::new();
        for cx in min_x..=max_x {
            for cy in min_y..=max_y {
                for cz in min_z..=max_z {
                    if let Some(indices) = self.cells.get(&(cx, cy, cz)) {
                        candidates.extend(indices.iter().copied());
                    }
                }
            }
        }
        candidates
    }

    fn max_radius(&self) -> f32 {
        self.max_radius
    }

    fn cell_size(&self) -> f32 {
        self.cell_size
    }
}

#[derive(Clone)]
pub enum SceneSound {
    Audible(AudibleSound),
    Sound3D(Sound3D),
    Pseudo3D(SoundPseudo3D),
}

impl SceneSound {
    pub fn id(&self) -> SoundObjectId {
        match self {
            SceneSound::Audible(s) => s.base.id,
            SceneSound::Sound3D(s) => s.base.base.id,
            SceneSound::Pseudo3D(s) => s.base.base.base.id,
        }
    }

    pub fn position(&self) -> Vector3 {
        match self {
            SceneSound::Audible(s) => s.position(),
            SceneSound::Sound3D(s) => s.position(),
            SceneSound::Pseudo3D(s) => s.base.position(),
        }
    }

    pub fn dropoff_radius(&self) -> Option<f32> {
        match self {
            SceneSound::Audible(s) => Some(s.dropoff_radius()),
            SceneSound::Sound3D(s) => Some(s.dropoff_radius_value()),
            SceneSound::Pseudo3D(s) => Some(s.dropoff_radius_value()),
        }
    }

    pub fn is_culled(&self) -> bool {
        match self {
            SceneSound::Audible(s) => s.base.is_culled(),
            SceneSound::Sound3D(s) => s.base.base.is_culled(),
            SceneSound::Pseudo3D(s) => s.base.base.base.is_culled(),
        }
    }

    pub fn set_culled(&mut self, culled: bool) {
        match self {
            SceneSound::Audible(s) => s.set_culled(culled),
            SceneSound::Sound3D(s) => s.set_culled(culled),
            SceneSound::Pseudo3D(s) => s.set_culled(culled),
        }
    }

    pub fn priority_value(&self) -> f32 {
        match self {
            SceneSound::Audible(s) => s.priority(),
            SceneSound::Sound3D(s) => s.get_priority(),
            SceneSound::Pseudo3D(s) => s.base.get_priority(),
        }
    }

    pub fn class_id(&self) -> crate::SoundClassId {
        match self {
            SceneSound::Audible(a) => a.base.class_id,
            SceneSound::Sound3D(s) => s.base.base.class_id,
            SceneSound::Pseudo3D(s) => s.base.base.base.class_id,
        }
    }

    pub fn is_static(&self) -> bool {
        match self {
            SceneSound::Audible(s) => s.base.is_static(),
            SceneSound::Sound3D(s) => s.is_static(),
            SceneSound::Pseudo3D(s) => s.base.is_static(),
        }
    }

    pub fn miles_handle(&self) -> Option<u32> {
        match self {
            SceneSound::Audible(s) => s
                .handle
                .as_ref()
                .and_then(|handle| handle.base.miles_handle()),
            SceneSound::Sound3D(s) => s
                .base
                .handle
                .as_ref()
                .and_then(|handle| handle.base.miles_handle()),
            SceneSound::Pseudo3D(s) => s
                .base
                .base
                .handle
                .as_ref()
                .and_then(|handle| handle.base.miles_handle()),
        }
    }
}

/// Audio scene manager.
#[derive(Clone)]
pub struct SoundScene {
    pub next_object_id: SoundObjectId,
    pub listener: Listener3D,
    pub secondary_listener: Option<Listener3D>,
    pub dynamic_sounds: Vec<SceneSound>,
    pub static_sounds: Vec<SceneSound>,
    pub logical_sounds: Vec<LogicalSound>,
    pub logical_listeners: Vec<LogicalListener>,
    pub logical_listener_cursor: usize,
    logical_index: LogicalSpatialIndex,
    logical_index_dirty: bool,
    pub batch_mode: bool,
    pub current_time_ms: u64,
    pub min_extents: Vector3,
    pub max_extents: Vector3,
}

impl SoundScene {
    pub fn new() -> Self {
        Self {
            next_object_id: 1,
            listener: Listener3D::new(0),
            secondary_listener: None,
            dynamic_sounds: Vec::new(),
            static_sounds: Vec::new(),
            logical_sounds: Vec::new(),
            logical_listeners: Vec::new(),
            logical_listener_cursor: 0,
            logical_index: LogicalSpatialIndex::new(100.0),
            logical_index_dirty: true,
            batch_mode: false,
            current_time_ms: 0,
            min_extents: Vector3::new(-500.0, -500.0, -500.0),
            max_extents: Vector3::new(500.0, 500.0, 500.0),
        }
    }

    pub fn allocate_id(&mut self) -> SoundObjectId {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.wrapping_add(1).max(1);
        id
    }

    pub fn set_batch_mode(&mut self, batch: bool) {
        self.batch_mode = batch;
    }

    pub fn is_batch_mode(&self) -> bool {
        self.batch_mode
    }

    pub fn set_listener_position(&mut self, position: Vector3) {
        self.listener.set_position(position);
    }

    pub fn listener_position(&self) -> Vector3 {
        self.listener.position()
    }

    pub fn set_listener_transform(&mut self, transform: Matrix3D) {
        self.listener.set_transform(transform);
    }

    pub fn listener_transform(&self) -> Matrix3D {
        self.listener.transform()
    }

    pub fn set_second_listener(&mut self, listener: Listener3D) {
        self.secondary_listener = Some(listener);
    }

    pub fn second_listener(&self) -> Option<&Listener3D> {
        self.secondary_listener.as_ref()
    }

    pub fn clear_second_listener(&mut self) {
        self.secondary_listener = None;
    }

    pub fn add_sound(&mut self, sound: SceneSound) {
        if sound.is_static() {
            self.static_sounds.push(sound);
        } else {
            self.dynamic_sounds.push(sound);
        }
    }

    pub fn add_dynamic_sound(&mut self, sound: SceneSound) {
        self.dynamic_sounds.push(sound);
    }

    pub fn add_static_sound(&mut self, mut sound: SceneSound) {
        match &mut sound {
            SceneSound::Audible(audible) => audible.mark_static(true),
            SceneSound::Sound3D(sound3d) => sound3d.make_static(true),
            SceneSound::Pseudo3D(pseudo) => pseudo.base.make_static(true),
        }
        self.static_sounds.push(sound);
    }

    pub fn remove_sound(&mut self, id: SoundObjectId) {
        self.dynamic_sounds.retain(|sound| sound.id() != id);
        self.static_sounds.retain(|sound| sound.id() != id);
    }

    pub fn remove_static_sound(&mut self, id: SoundObjectId) {
        self.static_sounds.retain(|sound| sound.id() != id);
    }

    pub fn add_logical_sound(&mut self, mut sound: LogicalSound) {
        sound.set_listener_timestamp(LogicalListener::newest_timestamp());
        if let Some(existing) = self
            .logical_sounds
            .iter_mut()
            .find(|entry| entry.base.id == sound.base.id)
        {
            *existing = sound;
        } else {
            self.logical_sounds.push(sound);
        }
        self.logical_index_dirty = true;
    }

    pub fn remove_logical_sound(&mut self, id: SoundObjectId) {
        self.logical_sounds.retain(|sound| sound.base.id != id);
        self.logical_index_dirty = true;
    }

    pub fn add_logical_listener(&mut self, listener: LogicalListener) {
        if let Some(existing) = self
            .logical_listeners
            .iter_mut()
            .find(|entry| entry.base.id == listener.base.id)
        {
            *existing = listener;
        } else {
            self.logical_listeners.push(listener);
        }
    }

    pub fn remove_logical_listener(&mut self, id: SoundObjectId) {
        self.logical_listeners
            .retain(|listener| listener.base.id != id);
        if self.logical_listeners.is_empty() {
            self.logical_listener_cursor = 0;
        } else {
            self.logical_listener_cursor %= self.logical_listeners.len();
        }
        self.logical_index_dirty = true;
    }

    pub fn update(&mut self, delta_ms: u32) {
        use std::cmp::Ordering;

        self.current_time_ms = self.current_time_ms.saturating_add(delta_ms as u64);
        let listener_position = self.listener.position();

        for sound in &mut self.dynamic_sounds {
            match sound {
                SceneSound::Audible(audible) => {
                    audible.update_fade(delta_ms);
                    audible.update_velocity_from_position(delta_ms as f32);
                }
                SceneSound::Sound3D(sound3d) => {
                    sound3d.on_frame_update(delta_ms);
                }
                SceneSound::Pseudo3D(pseudo) => {
                    pseudo.base.on_frame_update(delta_ms);
                }
            }

            if let Some(radius) = sound.dropoff_radius() {
                let distance_sq = sound.position().distance_squared(listener_position);
                let culled = distance_sq > radius * radius;
                sound.set_culled(culled);
            }

            match sound {
                SceneSound::Sound3D(sound3d) => {
                    sound3d.update_spatial_audio(&self.listener);
                }
                SceneSound::Pseudo3D(pseudo) => {
                    pseudo.update_spatial_audio(&self.listener);
                }
                SceneSound::Audible(_) => {}
            }
        }

        self.dynamic_sounds.sort_by(|a, b| {
            b.priority_value()
                .partial_cmp(&a.priority_value())
                .unwrap_or(Ordering::Equal)
        });

        self.logical_index_dirty = true;
    }

    pub fn collect_logical_sounds(&mut self, listener_count: Option<usize>) -> Vec<LogicalTrigger> {
        let total_listeners = self.logical_listeners.len();
        if total_listeners == 0 || self.logical_sounds.is_empty() {
            return Vec::new();
        }

        if self.logical_index_dirty {
            self.logical_index.rebuild(&self.logical_sounds);
            self.logical_index_dirty = false;
        }

        let requested = listener_count.unwrap_or(total_listeners);
        let limit = if requested == 0 {
            usize::MAX
        } else {
            requested
        };
        let listeners_to_process = requested
            .min(MAX_LOGICAL_LISTENER_UPDATES_PER_FRAME)
            .min(total_listeners);

        if listeners_to_process == 0 {
            return Vec::new();
        }

        let now = self.current_time_ms.min(u64::from(u32::MAX)) as u32;
        let mut collected = Vec::new();
        let mut per_sound_count: HashMap<SoundObjectId, usize> = HashMap::new();
        let mut single_shot_to_remove = HashSet::new();

        for offset in 0..listeners_to_process {
            let index = (self.logical_listener_cursor + offset) % total_listeners;
            if let Some(listener) = self.logical_listeners.get_mut(index) {
                LogicalListener::set_oldest_timestamp(listener.timestamp());
                listener.set_timestamp(LogicalListener::new_timestamp());

                let listener_position = listener.position();
                let effective_scale = listener.effective_scale();
                let search_radius = (self.logical_index.max_radius() * effective_scale)
                    .max(self.logical_index.cell_size());
                let candidates = self
                    .logical_index
                    .candidates(listener_position, search_radius);

                for idx in candidates {
                    if collected.len() >= limit {
                        break;
                    }
                    if let Some(sound) = self.logical_sounds.get_mut(idx) {
                        let listener_mask = listener.type_mask();
                        let sound_mask = sound.type_mask();
                        if sound_mask != 0
                            && listener_mask != 0
                            && (sound_mask & listener_mask) == 0
                        {
                            continue;
                        }
                        let max_listeners = sound.max_listeners();
                        let current_count = per_sound_count.entry(sound.base.id).or_default();
                        if max_listeners != 0 && *current_count >= max_listeners {
                            continue;
                        }
                        let effective_radius = sound.dropoff_radius() * effective_scale;
                        if effective_radius <= 0.0 {
                            continue;
                        }
                        let distance = sound.position().distance(listener_position);
                        if distance <= effective_radius && sound.allow_notify(now) {
                            sound.set_listener_timestamp(LogicalListener::new_timestamp());
                            *current_count += 1;
                            collected.push(LogicalTrigger {
                                listener_id: listener.base.id,
                                sound_id: sound.base.id,
                                type_mask: sound.type_mask(),
                                is_single_shot: sound.is_single_shot(),
                            });
                            if sound.is_single_shot() {
                                single_shot_to_remove.insert(sound.base.id);
                            }
                            if collected.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        self.logical_listener_cursor =
            (self.logical_listener_cursor + listeners_to_process) % total_listeners;

        if !single_shot_to_remove.is_empty() {
            self.logical_sounds
                .retain(|sound| !single_shot_to_remove.contains(&sound.base.id));
            if !single_shot_to_remove.is_empty() {
                self.logical_index_dirty = true;
            }
        }

        collected
    }

    pub fn flush_scene(&mut self) {
        self.dynamic_sounds.clear();
        self.static_sounds.clear();
        self.logical_sounds.clear();
        self.logical_index.cells.clear();
        self.logical_index.max_radius = 0.0;
        self.logical_listener_cursor = 0;
        self.logical_index_dirty = true;
    }

    pub fn find_sound(&self, id: SoundObjectId) -> Option<&SceneSound> {
        self.dynamic_sounds
            .iter()
            .chain(self.static_sounds.iter())
            .find(|sound| sound.id() == id)
    }

    pub fn find_sound_mut(&mut self, id: SoundObjectId) -> Option<&mut SceneSound> {
        if let Some(pos) = self
            .dynamic_sounds
            .iter()
            .position(|sound| sound.id() == id)
        {
            return self.dynamic_sounds.get_mut(pos);
        }
        self.static_sounds.iter_mut().find(|sound| sound.id() == id)
    }

    pub fn is_sound_in_scene(&self, id: SoundObjectId, include_static: bool) -> bool {
        self.dynamic_sounds.iter().any(|s| s.id() == id)
            || (include_static && self.static_sounds.iter().any(|s| s.id() == id))
    }
}
