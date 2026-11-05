//! Fixed, enum-ordered frame schedule (renderer plan R3/R4).
//!
//! A frame records its work phase by phase in [`FRAME_SCHEDULE`] order; within
//! one phase, work records in submission order. Phases mirror C++
//! `RTS3DScene::Flush` (W3DScene.cpp) plus W3DDisplay's 2D pass, so draw order
//! is decided here, not by the order in which subsystems happened to queue.

use crate::core::error::RendererResult;

/// One stage of the frame. Variant order is the C++ stage order; the
/// recording order is [`FRAME_SCHEDULE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FramePhase {
    /// Height map, roads, bibs (`RTS3DScene` terrain render). Clears the frame.
    Terrain,
    /// `DoShadows(false)`: projected/decal shadows and ground decals such as
    /// selection circles and order lines, under every object.
    DecalShadows,
    /// `DX8MeshRendererClass::Flush`: opaque and non-sorting blended meshes.
    /// Recorded by the renderer itself.
    Opaque,
    /// `flushOccludedObjectsIntoStencil`: occluded player-color silhouettes.
    OccludedStencil,
    /// `DoShadows(true)`: stencil volume shadows.
    StencilShadows,
    /// `Render_And_Clear_Static_Sort_Lists`: water.
    StaticSort,
    /// `flushTranslucentObjects`: translucent scene objects (laser lines).
    Translucent,
    /// `SortingRendererClass::Flush`: per-pass sorted translucent meshes.
    /// Recorded by the renderer itself.
    Sorted,
    /// `DoParticles`: particles, client decals, heat haze.
    Particles,
    /// W3DInGameUI 2D world overlays drawn before the window repaint
    /// (drag rectangle, health bars).
    Overlay2D,
}

impl FramePhase {
    /// Number of phases.
    pub const COUNT: usize = 10;

    /// Dense slot index for per-phase storage.
    pub const fn slot(self) -> usize {
        match self {
            Self::Terrain => 0,
            Self::DecalShadows => 1,
            Self::Opaque => 2,
            Self::OccludedStencil => 3,
            Self::StencilShadows => 4,
            Self::StaticSort => 5,
            Self::Translucent => 6,
            Self::Sorted => 7,
            Self::Particles => 8,
            Self::Overlay2D => 9,
        }
    }

    /// Phases recorded by the renderer itself rather than by queued work.
    pub const fn is_builtin(self) -> bool {
        matches!(self, Self::Opaque | Self::Sorted)
    }
}

/// Recording order of one frame. This is the order the FIFO callback queues
/// produced before the schedule existed (ground overlays, terrain, meshes,
/// sorting flush, then post-scene work in submission order); R4 moves it to
/// the C++ `RTS3DScene::Flush` order.
pub const FRAME_SCHEDULE: [FramePhase; FramePhase::COUNT] = [
    FramePhase::DecalShadows,
    FramePhase::Terrain,
    FramePhase::Opaque,
    FramePhase::Sorted,
    FramePhase::Overlay2D,
    FramePhase::OccludedStencil,
    FramePhase::Translucent,
    FramePhase::Particles,
    FramePhase::StencilShadows,
    FramePhase::StaticSort,
];

/// A native scene pass whose owner (Main's frame receiver) is borrowed only
/// while this frame executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedNativePass {
    TerrainGround,
    TerrainWater,
    ClientEffects,
}

pub(crate) type OwnedFrameWork =
    Box<dyn FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()>>;

pub(crate) enum FrameWork {
    Owned(OwnedFrameWork),
    ScopedNative(ScopedNativePass),
}

/// Work queued for the next frame, bucketed by phase.
#[derive(Default)]
pub(crate) struct FrameSchedule {
    slots: [Vec<FrameWork>; FramePhase::COUNT],
}

impl FrameSchedule {
    pub(crate) fn push(&mut self, phase: FramePhase, work: FrameWork) {
        debug_assert!(!phase.is_builtin(), "{phase:?} is recorded by the renderer");
        self.slots[phase.slot()].push(work);
    }

    pub(crate) fn take(&mut self, phase: FramePhase) -> Vec<FrameWork> {
        std::mem::take(&mut self.slots[phase.slot()])
    }

    pub(crate) fn queued(&self, phase: FramePhase) -> usize {
        self.slots[phase.slot()].len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.slots.iter().all(Vec::is_empty)
    }

    pub(crate) fn clear(&mut self) {
        self.slots.iter_mut().for_each(Vec::clear);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_lists_every_phase_once() {
        let mut slots: Vec<usize> = FRAME_SCHEDULE.iter().map(|phase| phase.slot()).collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..FramePhase::COUNT).collect::<Vec<_>>());
    }
}
