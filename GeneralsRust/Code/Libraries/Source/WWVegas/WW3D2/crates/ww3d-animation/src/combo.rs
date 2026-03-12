use glam::{Quat, Vec3};
use std::sync::Arc;

use crate::{HAnimClass, HTreeClass};

/// Per-pivot weight map used when blending animations.
#[derive(Debug, Clone)]
pub struct PivotWeightMap {
    weights: Vec<f32>,
}

impl PivotWeightMap {
    pub fn new(weights: Vec<f32>) -> Self {
        Self { weights }
    }

    pub fn len(&self) -> usize {
        self.weights.len()
    }

    pub fn element(&self, index: usize) -> f32 {
        self.weights.get(index).copied().unwrap_or(1.0)
    }

    pub fn element_mut(&mut self, index: usize) -> &mut f32 {
        if index >= self.weights.len() {
            self.weights.resize(index + 1, 1.0);
        }
        &mut self.weights[index]
    }
}

/// Named pivot map resolved lazily against an `HTreeClass`.
#[derive(Debug, Clone)]
pub struct NamedPivotMap {
    pending: Vec<(String, f32)>,
    resolved: PivotWeightMap,
}

impl NamedPivotMap {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            resolved: PivotWeightMap::new(Vec::new()),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, weight: f32) {
        self.pending.push((name.into(), weight));
    }

    pub fn resolve(&mut self, tree: &HTreeClass) {
        let mut weights = vec![1.0; tree.num_pivots()];
        for (name, weight) in &self.pending {
            if let Some(index) = tree.find_pivot_index(name) {
                if index < weights.len() {
                    weights[index] = *weight;
                }
            }
        }
        self.resolved = PivotWeightMap::new(weights);
    }

    pub fn map(&self) -> &PivotWeightMap {
        &self.resolved
    }
}

/// Pivot weight map variants.
#[derive(Debug, Clone)]
pub enum PivotMap {
    Direct(PivotWeightMap),
    Named(NamedPivotMap),
}

impl PivotMap {
    fn resolve_clone(&self, tree: Option<&HTreeClass>) -> PivotWeightMap {
        match self {
            PivotMap::Direct(map) => map.clone(),
            PivotMap::Named(named) => {
                let mut clone = named.clone();
                if let Some(tree) = tree {
                    clone.resolve(tree);
                }
                clone.map().clone()
            }
        }
    }
}

/// Per-animation combo data entry.
#[derive(Debug, Clone)]
pub struct HAnimComboData {
    shared: bool,
    motion: Option<Arc<HAnimClass>>,
    pivot_map: Option<PivotMap>,
    frame: f32,
    prev_frame: f32,
    weight: f32,
}

impl HAnimComboData {
    pub fn new(shared: bool) -> Self {
        Self {
            shared,
            motion: None,
            pivot_map: None,
            frame: 0.0,
            prev_frame: 0.0,
            weight: 1.0,
        }
    }

    pub fn is_shared(&self) -> bool {
        self.shared
    }

    pub fn set_motion(&mut self, motion: Option<Arc<HAnimClass>>) {
        self.motion = motion;
    }

    pub fn motion(&self) -> Option<&Arc<HAnimClass>> {
        self.motion.as_ref()
    }

    pub fn set_frame(&mut self, frame: f32) {
        self.prev_frame = self.frame;
        self.frame = frame;
    }

    pub fn advance_frame(&mut self, delta: f32) {
        self.prev_frame = self.frame;
        self.frame += delta;
    }

    pub fn frame(&self) -> f32 {
        self.frame
    }

    pub fn prev_frame(&self) -> f32 {
        self.prev_frame
    }

    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight.max(0.0);
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn set_pivot_map(&mut self, map: PivotMap) {
        self.pivot_map = Some(map);
    }

    pub fn pivot_map(&self) -> Option<&PivotMap> {
        self.pivot_map.as_ref()
    }

    pub fn resolved_pivot_map(&self, tree: Option<&HTreeClass>) -> Option<PivotWeightMap> {
        self.pivot_map.as_ref().map(|map| map.resolve_clone(tree))
    }

    pub fn set_resolved_pivot_map(&mut self, map: PivotWeightMap) {
        self.pivot_map = Some(PivotMap::Direct(map));
    }
}

/// Animation combo container matching the legacy `HAnimComboClass` behaviour.
#[derive(Debug, Default, Clone)]
pub struct HAnimCombo {
    entries: Vec<HAnimComboData>,
}

impl HAnimCombo {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    fn ensure_entry(&mut self, index: usize) -> &mut HAnimComboData {
        while self.entries.len() <= index {
            self.entries.push(HAnimComboData::new(false));
        }
        &mut self.entries[index]
    }

    pub fn num_anims(&self) -> usize {
        self.entries.len()
    }

    pub fn set_motion(&mut self, index: usize, motion: Option<Arc<HAnimClass>>) {
        self.ensure_entry(index).set_motion(motion);
    }

    pub fn get_motion(&self, index: usize) -> Option<Arc<HAnimClass>> {
        self.entries
            .get(index)
            .and_then(|entry| entry.motion())
            .cloned()
    }

    pub fn peek_motion(&self, index: usize) -> Option<&Arc<HAnimClass>> {
        self.entries.get(index).and_then(|entry| entry.motion())
    }

    pub fn set_frame(&mut self, index: usize, frame: f32) {
        self.ensure_entry(index).set_frame(frame);
    }

    pub fn get_frame(&self, index: usize) -> Option<f32> {
        self.entries.get(index).map(|entry| entry.frame())
    }

    pub fn set_prev_frame(&mut self, index: usize, frame: f32) {
        self.ensure_entry(index).prev_frame = frame;
    }

    pub fn get_prev_frame(&self, index: usize) -> Option<f32> {
        self.entries.get(index).map(|entry| entry.prev_frame())
    }

    pub fn set_weight(&mut self, index: usize, weight: f32) {
        self.ensure_entry(index).set_weight(weight);
    }

    pub fn get_weight(&self, index: usize) -> Option<f32> {
        self.entries.get(index).map(|entry| entry.weight())
    }

    pub fn set_pivot_weight_map(&mut self, index: usize, map: PivotWeightMap) {
        self.ensure_entry(index)
            .set_pivot_map(PivotMap::Direct(map));
    }

    pub fn set_named_pivot_weight_map(&mut self, index: usize, map: NamedPivotMap) {
        self.ensure_entry(index).set_pivot_map(PivotMap::Named(map));
    }

    pub fn pivot_weight_map(
        &self,
        index: usize,
        tree: Option<&HTreeClass>,
    ) -> Option<PivotWeightMap> {
        self.entries
            .get(index)
            .and_then(|entry| entry.resolved_pivot_map(tree))
    }

    pub fn append_entry(&mut self, entry: HAnimComboData) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[HAnimComboData] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut [HAnimComboData] {
        &mut self.entries
    }

    pub fn remove_entry(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Normalise weights either globally (no pivot maps) or per pivot (with pivot maps).
    pub fn normalize_weights(&mut self, tree: Option<&HTreeClass>) -> bool {
        if self.entries.is_empty() {
            return true;
        }

        let mut all_have_maps = true;
        let mut none_have_maps = true;
        let mut min_pivots = usize::MAX;

        for entry in &self.entries {
            if let Some(motion) = entry.motion() {
                min_pivots = min_pivots.min(motion.num_pivots());
            }
            if entry.pivot_map().is_some() {
                none_have_maps = false;
            } else {
                all_have_maps = false;
            }
        }

        if none_have_maps {
            let total: f32 = self
                .entries
                .iter()
                .filter(|entry| entry.motion().is_some())
                .map(|entry| entry.weight())
                .sum();
            if total.abs() <= f32::EPSILON {
                return true;
            }
            let inv_total = 1.0 / total;
            for entry in &mut self.entries {
                if entry.motion().is_some() {
                    let new_weight = entry.weight() * inv_total;
                    entry.set_weight(new_weight);
                }
            }
            true
        } else if all_have_maps {
            let tree = match tree {
                Some(tree) => tree,
                None => return false,
            };

            let mut resolved_maps: Vec<Option<PivotWeightMap>> = self
                .entries
                .iter()
                .map(|entry| entry.resolved_pivot_map(Some(tree)))
                .collect();

            for pivot in 1..min_pivots {
                let mut total = 0.0f32;
                for (entry, map_opt) in self.entries.iter().zip(resolved_maps.iter()) {
                    if let (Some(map), Some(_motion)) = (map_opt, entry.motion()) {
                        total += entry.weight() * map.element(pivot);
                    }
                }

                if total.abs() > f32::EPSILON && (total - 1.0).abs() > f32::EPSILON {
                    let inv_total = 1.0 / total;
                    for map_opt in resolved_maps.iter_mut() {
                        if let Some(map) = map_opt {
                            let new_weight = map.element(pivot) * inv_total;
                            *map.element_mut(pivot) = new_weight;
                        }
                    }
                }
            }

            for (entry, map_opt) in self.entries.iter_mut().zip(resolved_maps.into_iter()) {
                if let Some(map) = map_opt {
                    entry.set_resolved_pivot_map(map);
                }
            }
            true
        } else {
            false
        }
    }

    /// Blend the registered animations into the provided buffers.
    pub fn blend_into(
        &self,
        tree: &HTreeClass,
        translations: &mut [Vec3],
        rotations: &mut [Quat],
        mut visibility: Option<&mut [bool]>,
    ) {
        let pivot_count = tree.num_pivots();
        for vec in translations.iter_mut().take(pivot_count) {
            *vec = Vec3::ZERO;
        }
        for quat in rotations.iter_mut().take(pivot_count) {
            *quat = Quat::IDENTITY;
        }
        if let Some(flags) = visibility.as_mut() {
            for vis in (*flags).iter_mut().take(pivot_count) {
                *vis = true;
            }
        }

        let mut translation_weights = vec![0.0f32; pivot_count];
        let mut rotation_weights = vec![0.0f32; pivot_count];

        for entry in &self.entries {
            let Some(motion) = entry.motion() else {
                continue;
            };
            let weight = entry.weight();
            if weight <= 0.0 {
                continue;
            }

            let per_pivot_weights = entry.resolved_pivot_map(Some(tree));

            for pivot in 0..pivot_count {
                let pivot_weight = per_pivot_weights
                    .as_ref()
                    .map(|map| map.element(pivot))
                    .unwrap_or(1.0);
                if pivot_weight <= 0.0 {
                    continue;
                }
                let total_weight = weight * pivot_weight;
                if total_weight <= 0.0 {
                    continue;
                }

                let translation = motion.get_translation(pivot, entry.frame());
                translations[pivot] += translation * total_weight;
                translation_weights[pivot] += total_weight;

                let rotation = motion.get_orientation(pivot, entry.frame());
                let current = rotations[pivot];
                let accum_weight = rotation_weights[pivot];
                if accum_weight <= f32::EPSILON {
                    rotations[pivot] = rotation;
                    rotation_weights[pivot] = total_weight;
                } else {
                    let factor = total_weight / (accum_weight + total_weight);
                    rotations[pivot] = current.slerp(rotation, factor).normalize();
                    rotation_weights[pivot] += total_weight;
                }

                if let Some(flags) = visibility.as_mut() {
                    if let Some(vis) = (*flags).get_mut(pivot) {
                        let visible = motion.get_visibility(pivot, entry.frame());
                        *vis &= visible;
                    }
                }
            }
        }

        for (index, sum) in translation_weights.iter().enumerate() {
            if *sum > f32::EPSILON {
                translations[index] /= *sum;
            }
        }
    }
}
