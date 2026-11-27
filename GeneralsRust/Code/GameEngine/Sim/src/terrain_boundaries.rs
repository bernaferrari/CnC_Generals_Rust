//! Ordered map extents and active-boundary identity for the receiving terrain.

use crate::terrain::MAP_XY_FACTOR;
use glam::{IVec2, Vec2};

#[derive(Debug, Default)]
pub struct TerrainBoundaries {
    authored: Vec<IVec2>,
    active: i32,
}

impl TerrainBoundaries {
    /// Installing map geometry keeps the receiving boundary selection until reset.
    pub fn replace(&mut self, authored: Vec<IVec2>) {
        self.authored = authored;
    }

    pub fn clear(&mut self) {
        self.authored.clear();
        self.active = 0;
    }

    pub fn active(&self) -> i32 {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.authored.is_empty()
    }

    pub fn extent(&self) -> Vec2 {
        self.authored
            .get(self.active.max(0) as usize)
            .copied()
            .unwrap_or(IVec2::ZERO)
            .as_vec2()
            * MAP_XY_FACTOR
    }

    pub fn maximum_extent(&self) -> Vec2 {
        let mut maximum = Vec2::ZERO;
        for boundary in &self.authored {
            maximum = maximum.max(boundary.as_vec2() * MAP_XY_FACTOR);
        }
        maximum
    }

    /// Resolve a candidate without moving the active boundary.
    pub fn extent_for_saved_boundary(&self, index: i32) -> Option<Vec2> {
        if index == self.active {
            return Some(self.extent());
        }
        Some(self.valid_boundary(index)?.as_vec2() * MAP_XY_FACTOR)
    }

    pub fn restore(&mut self, index: i32) {
        if self.valid_boundary(index).is_some() {
            self.active = index;
        }
    }

    fn valid_boundary(&self, index: i32) -> Option<IVec2> {
        let boundary = *self.authored.get(usize::try_from(index).ok()?)?;
        (boundary.x != 0 && boundary.y != 0).then_some(boundary)
    }
}
