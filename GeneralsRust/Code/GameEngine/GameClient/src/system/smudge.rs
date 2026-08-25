//! Smudge system (terrain decals), matching System/Smudge.cpp.

use crate::effects::decals::DecalRenderItem;
use glam::{Vec2, Vec3};
use nalgebra::Point3;
use std::sync::{Arc, Mutex, OnceLock};

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

    pub fn reset(&mut self) {
        while let Some(smudge) = self.used.pop() {
            push_free_smudge(smudge);
        }
    }

    pub fn add_smudge_to_set(&mut self) -> &mut Smudge {
        let smudge = pop_free_smudge().unwrap_or_default();
        self.used.push(smudge);
        let index = self.used.len().saturating_sub(1);
        &mut self.used[index]
    }

    pub fn remove_smudge_from_set(&mut self, index: usize) {
        if index < self.used.len() {
            let smudge = self.used.swap_remove(index);
            push_free_smudge(smudge);
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
    used_sets: Vec<SmudgeSetHandle>,
    free_sets: Vec<SmudgeSetHandle>,
    smudge_count_last_frame: i32,
    hardware_support: HardwareSmudgeSupport,
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
            smudge_count_last_frame: 0,
            hardware_support: HardwareSmudgeSupport::Unknown,
        }
    }

    pub fn init(&mut self) {
        self.hardware_support = HardwareSmudgeSupport::Yes;
    }

    pub fn reset(&mut self) {
        while let Some(set) = self.used_sets.pop() {
            if let Ok(mut guard) = set.lock() {
                guard.reset();
            }
            self.free_sets.push(set);
        }
    }

    pub fn add_smudge_set(&mut self) -> SmudgeSetHandle {
        let set = if let Some(set) = self.free_sets.pop() {
            set
        } else {
            Arc::new(Mutex::new(SmudgeSet::new()))
        };
        self.used_sets.push(Arc::clone(&set));
        set
    }

    pub fn last_used_set(&self) -> Option<SmudgeSetHandle> {
        self.used_sets.last().cloned()
    }

    pub fn remove_smudge_set(&mut self, set: &SmudgeSetHandle) {
        if let Some(pos) = self
            .used_sets
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, set))
        {
            let set = self.used_sets.swap_remove(pos);
            self.free_sets.push(set);
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
        for set in &self.used_sets {
            if let Ok(guard) = set.lock() {
                items.extend(guard.used_smudges().iter().cloned());
            }
        }
        items
    }

    pub fn collect_decal_render_items(&self) -> Vec<DecalRenderItem> {
        self.collect_used_smudges()
            .into_iter()
            .filter(|smudge| smudge.size > 0.0 && smudge.opacity > 0.0)
            .map(|smudge| DecalRenderItem {
                position: Point3::new(smudge.pos.x, smudge.pos.y, smudge.pos.z),
                size: smudge.size,
                size_x: smudge.size,
                size_y: smudge.size,
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, smudge.opacity],
                texture_name: String::new(),
                shadow_type: 0,
            })
            .collect()
    }
}

pub type SmudgeSetHandle = Arc<Mutex<SmudgeSet>>;

static THE_SMUDGE_MANAGER: OnceLock<Mutex<SmudgeManager>> = OnceLock::new();

pub fn get_smudge_manager() -> &'static Mutex<SmudgeManager> {
    THE_SMUDGE_MANAGER.get_or_init(|| Mutex::new(SmudgeManager::new()))
}

static FREE_SMUDGES: OnceLock<Mutex<Vec<Smudge>>> = OnceLock::new();

fn free_smudge_pool() -> &'static Mutex<Vec<Smudge>> {
    FREE_SMUDGES.get_or_init(|| Mutex::new(Vec::new()))
}

fn pop_free_smudge() -> Option<Smudge> {
    free_smudge_pool()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pop()
}

fn push_free_smudge(smudge: Smudge) {
    free_smudge_pool()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(smudge);
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
    let Some(set) = manager.used_sets.first().cloned() else {
        return false;
    };
    // Drop manager before locking set to avoid lock-order inversion with reset().
    drop(manager);
    let Ok(mut guard) = set.lock() else {
        return false;
    };
    let smudge = guard.add_smudge_to_set();
    smudge.size = size;
    smudge.opacity = opacity;
    let count = guard.used_smudge_count();
    drop(guard);
    RESIDUAL_SMUDGE_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SMUDGE_SET_COUNT.store(1, std::sync::atomic::Ordering::Relaxed);
    residual_smudge_action_store(ResidualSmudgeAction::AddSmudge);
    count > 0
}

/// Residual: remove first residual smudge.
pub fn simulate_smudge_remove_first() -> bool {
    let Ok(manager) = get_smudge_manager().lock() else {
        return false;
    };
    let Some(set) = manager.used_sets.first().cloned() else {
        return false;
    };
    drop(manager);
    let Ok(mut guard) = set.lock() else {
        return false;
    };
    if guard.used_smudge_count() == 0 {
        return false;
    }
    guard.remove_smudge_from_set(0);
    let count = guard.used_smudge_count();
    drop(guard);
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

        {
            let mut guard = set.lock().unwrap();
            guard.add_smudge_to_set().size = 42.0;
        }

        manager.remove_smudge_set(&set);
        assert_eq!(set.lock().unwrap().used_smudge_count(), 1);

        let reused = manager.add_smudge_set();
        assert!(Arc::ptr_eq(&set, &reused));

        let guard = reused.lock().unwrap();
        assert_eq!(guard.used_smudge_count(), 1);
        assert_eq!(guard.used_smudges()[0].size, 42.0);
    }

    /// Residual smudges must become GPU decal items so the live frame can
    /// draw them via `ParticleRenderer::render_decals` (C++ W3DSmudgeManager).
    #[test]
    fn collect_decal_render_items_skips_empty_and_keeps_used() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();
        {
            let mut guard = set.lock().unwrap();
            let drawn = guard.add_smudge_to_set();
            drawn.pos = Vec3::new(4.0, 5.0, 6.0);
            drawn.size = 8.0;
            drawn.opacity = 0.5;
            let skipped = guard.add_smudge_to_set();
            skipped.size = 0.0;
            skipped.opacity = 1.0;
        }
        let items = manager.collect_decal_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Point3::new(4.0, 5.0, 6.0));
        assert_eq!(items[0].size, 8.0);
        assert!((items[0].color[3] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clears_used_sets_before_pooling() {
        let mut manager = SmudgeManager::new();
        let set = manager.add_smudge_set();
        set.lock().unwrap().add_smudge_to_set().size = 12.0;

        manager.reset();

        let reused = manager.add_smudge_set();
        assert!(Arc::ptr_eq(&set, &reused));
        assert_eq!(reused.lock().unwrap().used_smudge_count(), 0);
    }
}
