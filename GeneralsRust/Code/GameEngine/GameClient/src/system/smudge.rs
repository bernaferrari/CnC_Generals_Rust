//! Smudge system (terrain decals), matching System/Smudge.cpp.

use crate::effects::decals::DecalRenderItem;
use glam::{Vec2, Vec3};
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Copy)]
pub struct SmudgeVertex {
    pub pos: Vec3,
    pub uv: Vec2,
}

impl Default for SmudgeVertex {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            uv: Vec2::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Smudge {
    pub pos: Vec3,
    pub offset: Vec2,
    pub size: f32,
    pub opacity: f32,
    pub verts: [SmudgeVertex; 5],
}

impl Default for Smudge {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            offset: Vec2::ZERO,
            size: 0.0,
            opacity: 1.0,
            verts: [SmudgeVertex::default(); 5],
        }
    }
}

#[derive(Debug, Default)]
pub struct SmudgeSet {
    used: Vec<Smudge>,
}

impl SmudgeSet {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self, free_pool: &mut Vec<Smudge>) {
        // C++ removes used smudges from the head and adds each to the free head.
        // Appending in used order gives the same next-reused smudge with Vec::pop.
        for smudge in self.used.drain(..) {
            free_pool.push(smudge);
        }
    }

    fn add_smudge_to_set(&mut self, free_pool: &mut Vec<Smudge>) -> &mut Smudge {
        let smudge = free_pool.pop().unwrap_or_default();
        self.used.push(smudge);
        self.used.last_mut().expect("just pushed")
    }

    fn remove_smudge_from_set(&mut self, index: usize, free_pool: &mut Vec<Smudge>) {
        if index < self.used.len() {
            // C++ intrusive-list removal keeps the remaining used order.
            let smudge = self.used.remove(index);
            free_pool.push(smudge);
        }
    }

    pub fn used_smudges(&self) -> &[Smudge] {
        &self.used
    }

    pub fn used_smudge_count(&self) -> usize {
        self.used.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardwareSmudgeSupport {
    Unknown,
    No,
    Yes,
}

#[derive(Debug)]
pub struct SmudgeManager {
    used_sets: Vec<ManagedSmudgeSet>,
    free_sets: Vec<ManagedSmudgeSet>,
    free_smudges: Vec<Smudge>,
    next_set_id: u64,
    smudge_count_last_frame: i32,
    hardware_support: HardwareSmudgeSupport,
}

#[derive(Debug)]
struct ManagedSmudgeSet {
    id: u64,
    set: SmudgeSet,
}

/// Stable identity for a `SmudgeSet` owned by the `SmudgeManager`
/// (C++ hands out raw `SmudgeSet*`; this port uses manager-scoped ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmudgeSetHandle {
    id: u64,
}

impl Default for SmudgeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SmudgeManager {
    pub fn new() -> Self {
        Self {
            used_sets: Vec::new(),
            free_sets: Vec::new(),
            free_smudges: Vec::new(),
            next_set_id: 0,
            smudge_count_last_frame: 0,
            hardware_support: HardwareSmudgeSupport::Unknown,
        }
    }

    pub fn init(&mut self) {
        self.hardware_support = HardwareSmudgeSupport::Yes;
    }

    pub fn reset(&mut self) {
        while let Some(mut entry) = self.used_sets.pop() {
            entry.set.reset(&mut self.free_smudges);
            self.free_sets.push(entry);
        }
    }

    pub fn add_smudge_set(&mut self) -> SmudgeSetHandle {
        let mut entry = self.free_sets.pop().unwrap_or_else(|| {
            self.next_set_id += 1;
            ManagedSmudgeSet {
                id: self.next_set_id,
                set: SmudgeSet::new(),
            }
        });
        let handle = SmudgeSetHandle { id: entry.id };
        self.used_sets.push(entry);
        handle
    }

    pub fn last_used_set(&self) -> Option<SmudgeSetHandle> {
        self.used_sets
            .last()
            .map(|entry| SmudgeSetHandle { id: entry.id })
    }

    pub fn first_used_set(&self) -> Option<SmudgeSetHandle> {
        self.used_sets
            .first()
            .map(|entry| SmudgeSetHandle { id: entry.id })
    }

    pub fn remove_smudge_set(&mut self, set: &SmudgeSetHandle) {
        if let Some(pos) = self
            .used_sets
            .iter()
            .position(|candidate| candidate.id == set.id)
        {
            let entry = self.used_sets.swap_remove(pos);
            self.free_sets.push(entry);
        }
    }

    pub fn smudge_set_used_smudges(&self, handle: &SmudgeSetHandle) -> Option<&[Smudge]> {
        self.used_sets
            .iter()
            .chain(self.free_sets.iter())
            .find(|entry| entry.id == handle.id)
            .map(|entry| entry.set.used_smudges())
    }

    pub fn reset_smudge_set(&mut self, handle: &SmudgeSetHandle) {
        let Self {
            used_sets,
            free_sets,
            free_smudges,
            ..
        } = self;
        if let Some(entry) = used_sets
            .iter_mut()
            .chain(free_sets.iter_mut())
            .find(|entry| entry.id == handle.id)
        {
            entry.set.reset(free_smudges);
        }
    }

    pub fn smudge_set_used_count(&self, handle: &SmudgeSetHandle) -> Option<usize> {
        self.used_sets
            .iter()
            .chain(self.free_sets.iter())
            .find(|entry| entry.id == handle.id)
            .map(|entry| entry.set.used_smudge_count())
    }

    pub fn add_smudge_to_set(&mut self, handle: &SmudgeSetHandle) -> Option<&mut Smudge> {
        let Self {
            used_sets,
            free_sets,
            free_smudges,
            ..
        } = self;
        let set = used_sets
            .iter_mut()
            .chain(free_sets.iter_mut())
            .find(|entry| entry.id == handle.id)
            .map(|entry| &mut entry.set)?;
        Some(set.add_smudge_to_set(free_smudges))
    }

    pub fn remove_smudge_from_set(&mut self, handle: &SmudgeSetHandle, index: usize) {
        let Self {
            used_sets,
            free_sets,
            free_smudges,
            ..
        } = self;
        if let Some(entry) = used_sets
            .iter_mut()
            .chain(free_sets.iter_mut())
            .find(|entry| entry.id == handle.id)
        {
            entry.set.remove_smudge_from_set(index, free_smudges);
        }
    }

    pub fn get_smudge_count_last_frame(&self) -> i32 {
        self.smudge_count_last_frame
    }

    pub fn set_smudge_count_last_frame(&mut self, count: i32) {
        self.smudge_count_last_frame = count;
    }

    pub fn get_hardware_support(&self) -> bool {
        self.hardware_support != HardwareSmudgeSupport::No
    }

    /// Cheap terrain-decal representation of residual smudges.
    /// C++ `W3DSmudgeManager` draws heat-distortion textures; until that
    /// post-process exists, used smudges are issued as `DecalRenderItem`s.
    pub fn collect_used_smudges(&self) -> Vec<Smudge> {
        let mut items = Vec::new();
        for entry in &self.used_sets {
            items.extend(entry.set.used_smudges().iter().cloned());
        }
        items
    }

    pub fn collect_decal_render_items(&self) -> Vec<DecalRenderItem> {
        self.collect_used_smudges()
            .into_iter()
            .filter(|smudge| smudge.size > 0.0 && smudge.opacity > 0.0)
            .map(|smudge| DecalRenderItem {
                position: Vec3::new(smudge.pos.x, smudge.pos.y, smudge.pos.z),
                size: smudge.size,
                size_x: smudge.size,
                size_y: smudge.size,
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, smudge.opacity],
                texture_name: String::new(),
                shadow_type: 0,
                uv_offset: [0.0, 0.0],
            })
            .collect()
    }
}

static THE_SMUDGE_MANAGER: LazyLock<Mutex<SmudgeManager>> =
    LazyLock::new(|| Mutex::new(SmudgeManager::new()));

pub fn get_smudge_manager() -> &'static Mutex<SmudgeManager> {
    &THE_SMUDGE_MANAGER
}

/// Residual: last Smudge action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualSmudgeAction {
    None = 0,
    AddSet = 1,
    AddSmudge = 2,
    RemoveSmudge = 3,
    RemoveSet = 4,
    Reset = 5,
    SetCount = 6,
}

static RESIDUAL_SMUDGE_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SMUDGE_SET_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_SMUDGE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn residual_smudge_action_store(action: ResidualSmudgeAction) {
    RESIDUAL_SMUDGE_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last Smudge residual action.
pub fn residual_smudge_last_action() -> ResidualSmudgeAction {
    match RESIDUAL_SMUDGE_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualSmudgeAction::AddSet,
        2 => ResidualSmudgeAction::AddSmudge,
        3 => ResidualSmudgeAction::RemoveSmudge,
        4 => ResidualSmudgeAction::RemoveSet,
        5 => ResidualSmudgeAction::Reset,
        6 => ResidualSmudgeAction::SetCount,
        _ => ResidualSmudgeAction::None,
    }
}

/// Residual: residual smudge-set count latch.
pub fn residual_smudge_set_count() -> usize {
    RESIDUAL_SMUDGE_SET_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: residual smudge count latch inside residual set.
pub fn residual_smudge_count() -> usize {
    RESIDUAL_SMUDGE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: allocate a residual smudge set without terrain render.
/// Uses only SmudgeManager lock (no nested residual set slot).
pub fn simulate_smudge_add_set() -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    // Keep a single residual set: clear used sets first for deterministic residual.
    manager.reset();
    let _set = manager.add_smudge_set();
    RESIDUAL_SMUDGE_SET_COUNT.store(1, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SMUDGE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::AddSet);
    residual_smudge_set_count() == 1
}

/// Residual: add a smudge into the first residual set.
pub fn simulate_smudge_add(size: f32, opacity: f32) -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    if manager.used_sets.is_empty() {
        let _ = manager.add_smudge_set();
        RESIDUAL_SMUDGE_SET_COUNT.store(1, std::sync::atomic::Ordering::Relaxed);
    }
    let Some(set) = manager.first_used_set() else {
        return false;
    };
    let count = {
        let Some(smudge) = manager.add_smudge_to_set(&set) else {
            return false;
        };
        smudge.size = size;
        smudge.opacity = opacity;
        manager
            .smudge_set_used_count(&set)
            .unwrap_or_default()
    };
    RESIDUAL_SMUDGE_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SMUDGE_SET_COUNT.store(1, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::AddSmudge);
    count > 0
}

/// Residual: remove first residual smudge.
pub fn simulate_smudge_remove_first() -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    let Some(set) = manager.first_used_set() else {
        return false;
    };
    if manager.smudge_set_used_count(&set).unwrap_or(0) == 0 {
        return false;
    }
    manager.remove_smudge_from_set(&set, 0);
    let count = manager.smudge_set_used_count(&set).unwrap_or(0);
    RESIDUAL_SMUDGE_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::RemoveSmudge);
    true
}

/// Residual: remove residual smudge set(s).
pub fn simulate_smudge_remove_set() -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    manager.reset();
    RESIDUAL_SMUDGE_SET_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SMUDGE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::RemoveSet);
    residual_smudge_set_count() == 0
}

/// Residual: reset smudge manager residual.
pub fn simulate_smudge_reset() -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    manager.reset();
    RESIDUAL_SMUDGE_SET_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SMUDGE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::Reset);
    residual_smudge_set_count() == 0 && residual_smudge_count() == 0
}

/// Residual: set last-frame smudge count residual.
pub fn simulate_smudge_set_count_last_frame(count: i32) -> bool {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return false;
    };
    manager.set_smudge_count_last_frame(count);
    residual_smudge_action_store(ResidualSmudgeAction::SetCount);
    manager.get_smudge_count_last_frame() == count
}

/// Residual: add set + smudge composite.
pub fn simulate_smudge_prepare_set_with_smudge(size: f32) -> bool {
    if !simulate_smudge_add_set() {
        return false;
    }
    simulate_smudge_add(size, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_smudge_set_reuses_without_reset_like_cpp() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();

        manager.add_smudge_to_set(&set).unwrap().size = 42.0;

        manager.remove_smudge_set(&set);
        assert_eq!(manager.smudge_set_used_count(&set), Some(1));

        let reused = manager.add_smudge_set();
        assert_eq!(set, reused);

        assert_eq!(manager.smudge_set_used_count(&reused), Some(1));
        assert_eq!(
            manager.smudge_set_used_smudges(&reused).unwrap()[0].size,
            42.0
        );
    }

    /// Residual smudges must become GPU decal items so the live frame can
    /// draw them via `ParticleRenderer::render_decals` (C++ W3DSmudgeManager).
    #[test]
    fn collect_decal_render_items_skips_empty_and_keeps_used() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();
        let drawn = manager.add_smudge_to_set(&set).unwrap();
        drawn.pos = Vec3::new(4.0, 5.0, 6.0);
        drawn.size = 8.0;
        drawn.opacity = 0.5;
        let skipped = manager.add_smudge_to_set(&set).unwrap();
        skipped.size = 0.0;
        skipped.opacity = 1.0;
        let items = manager.collect_decal_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(items[0].size, 8.0);
        assert!((items[0].color[3] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clears_used_sets_before_pooling() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();
        manager.add_smudge_to_set(&set).unwrap().size = 12.0;

        manager.reset();

        let reused = manager.add_smudge_set();
        assert_eq!(set, reused);
        assert_eq!(manager.smudge_set_used_count(&reused), Some(0));
    }

    #[test]
    fn free_smudges_follow_cpp_reuse_order_within_one_manager() {
        let mut manager = SmudgeManager::new();
        let first_set = manager.add_smudge_set();
        manager.add_smudge_to_set(&first_set).unwrap().size = 1.0;
        manager.add_smudge_to_set(&first_set).unwrap().size = 2.0;
        manager.reset_smudge_set(&first_set);

        let second_set = manager.add_smudge_set();
        assert_eq!(manager.add_smudge_to_set(&second_set).unwrap().size, 2.0);
        assert_eq!(manager.add_smudge_to_set(&second_set).unwrap().size, 1.0);
    }

    #[test]
    fn removing_a_smudge_keeps_the_remaining_cpp_list_order() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();
        for size in [1.0, 2.0, 3.0] {
            manager.add_smudge_to_set(&set).unwrap().size = size;
        }

        manager.remove_smudge_from_set(&set, 1);
        assert_eq!(
            manager
                .smudge_set_used_smudges(&set)
                .unwrap()
                .iter()
                .map(|smudge| smudge.size)
                .collect::<Vec<_>>(),
            vec![1.0, 3.0]
        );
        assert_eq!(manager.add_smudge_to_set(&set).unwrap().size, 2.0);
    }

    #[test]
    fn free_smudges_do_not_leak_between_managers() {
        let mut first_manager = SmudgeManager::new();
        let first_set = first_manager.add_smudge_set();
        first_manager.add_smudge_to_set(&first_set).unwrap().size = 42.0;
        first_manager.reset_smudge_set(&first_set);

        let mut second_manager = SmudgeManager::new();
        let second_set = second_manager.add_smudge_set();
        assert_eq!(
            second_manager
                .add_smudge_to_set(&second_set)
                .unwrap()
                .size,
            0.0
        );
        assert_eq!(
            first_manager.add_smudge_to_set(&first_set).unwrap().size,
            42.0
        );
    }
}
