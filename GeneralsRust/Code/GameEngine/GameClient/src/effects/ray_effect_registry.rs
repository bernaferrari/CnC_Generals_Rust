//! CPP RayEffect.cpp bookkeeping owned by the receiving GameClient.
//!
//! The registry never creates, fades or destroys a Drawable. LaserUpdate and
//! the Drawable owner control the actual visual lifecycle.
use crate::drawable::DrawableId;
use gamelogic::common::Coord3D;

pub const MAX_RAY_EFFECTS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayEffectData {
    pub drawable: DrawableId,
    pub start: Coord3D,
    pub end: Coord3D,
}

pub struct RayEffectSystem {
    slots: [Option<RayEffectData>; MAX_RAY_EFFECTS],
}
impl Default for RayEffectSystem {
    fn default() -> Self {
        Self {
            slots: [None; MAX_RAY_EFFECTS],
        }
    }
}
impl RayEffectSystem {
    pub fn reset(&mut self) {
        self.slots.fill(None);
    }
    /// Original first free slot; full admission leaves the Drawable alive.
    pub fn add(&mut self, drawable: DrawableId, start: Coord3D, end: Coord3D) -> bool {
        if drawable == DrawableId::INVALID {
            return false;
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some(RayEffectData {
            drawable,
            start,
            end,
        });
        true
    }
    /// Original findEntry/deleteRayEffect removes the first matching entry.
    pub fn delete(&mut self, drawable: DrawableId) -> bool {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|data| data.drawable == drawable))
        else {
            return false;
        };
        *slot = None;
        true
    }
    pub fn get(&self, drawable: DrawableId) -> Option<&RayEffectData> {
        self.slots
            .iter()
            .flatten()
            .find(|data| data.drawable == drawable)
    }
    pub fn iter(&self) -> impl Iterator<Item = &RayEffectData> {
        self.slots.iter().flatten()
    }
}
