//! Smudge system (terrain decals), matching System/Smudge.cpp.

use crate::effects::decals::DecalRenderItem;
use glam::{Vec2, Vec3};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy)]
pub struct SmudgeVertex {
    pub pos: Vec3,
    pub uv: Vec2,
}

#[cfg(test)]
#[path = "smudge_order_tests.rs"]
mod order_tests;

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

/// Set storage is owned by one manager; mutable access borrows that manager.
#[derive(Debug, Default)]
pub struct SmudgeSet {
    used: Vec<Smudge>,
}

impl SmudgeSet {
    pub fn used_smudges(&self) -> &[Smudge] {
        &self.used
    }

    pub fn used_smudge_count(&self) -> usize {
        self.used.len()
    }
}

/// Exclusive access to a set and its manager's shared recycling pool.
///
/// The borrow cannot survive a manager reset or escape into a second owner.
/// ```compile_fail,E0499
/// use game_client_rust::system::smudge::SmudgeManager;
/// let mut manager = SmudgeManager::new();
/// let mut set = manager.add_smudge_set();
/// manager.reset();
/// set.add_smudge_to_set();
/// ```
pub struct SmudgeSetMut<'a> {
    manager: &'a mut SmudgeManager,
    index: usize,
}

impl std::ops::Deref for SmudgeSetMut<'_> {
    type Target = SmudgeSet;

    fn deref(&self) -> &Self::Target {
        &self.manager.used_sets[self.index]
    }
}

impl SmudgeSetMut<'_> {
    pub fn reset(&mut self) {
        // C++ removes used head and adds to free head, represented by Vec::pop.
        self.manager
            .free_smudges
            .extend(self.manager.used_sets[self.index].used.drain(..));
    }

    pub fn add_smudge_to_set(&mut self) -> &mut Smudge {
        let smudge = self.manager.free_smudges.pop().unwrap_or_default();
        let used = &mut self.manager.used_sets[self.index].used;
        used.push(smudge);
        used.last_mut().expect("just inserted a smudge")
    }

    pub fn remove_smudge_from_set(&mut self, index: usize) {
        let used = &mut self.manager.used_sets[self.index].used;
        if index < used.len() {
            // Intrusive-list removal leaves the remaining smudges in order.
            self.manager.free_smudges.push(used.remove(index));
        }
    }

    /// Return this set to the free head without resetting its smudges (C++).
    pub fn remove(self) {
        let set = self.manager.used_sets.remove(self.index);
        self.manager.free_sets.push(set);
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
    // Boxes preserve C++ set identity when moving between the used/free lists.
    used_sets: Vec<Box<SmudgeSet>>,
    free_sets: Vec<Box<SmudgeSet>>,
    free_smudges: Vec<Smudge>,
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
            free_smudges: Vec::new(),
            smudge_count_last_frame: 0,
            hardware_support: HardwareSmudgeSupport::Unknown,
        }
    }

    pub fn init(&mut self) {
        self.hardware_support = HardwareSmudgeSupport::Yes;
    }

    pub fn reset(&mut self) {
        // Used head first; append to free tail behind any already-free sets.
        for set in &mut self.used_sets {
            self.free_smudges.extend(set.used.drain(..));
        }
        self.free_sets.splice(0..0, self.used_sets.drain(..).rev());
    }

    pub fn add_smudge_set(&mut self) -> SmudgeSetMut<'_> {
        let set = self.free_sets.pop().unwrap_or_default();
        self.used_sets.push(set);
        self.borrow_set(self.used_sets.len() - 1)
    }

    pub fn last_used_set(&mut self) -> Option<SmudgeSetMut<'_>> {
        let index = self.used_sets.len().checked_sub(1)?;
        Some(self.borrow_set(index))
    }

    /// The particle feed may precede frame setup; retain its lazy first set.
    pub fn current_smudge_set(&mut self) -> SmudgeSetMut<'_> {
        if self.used_sets.is_empty() {
            let _ = self.add_smudge_set();
        }
        self.last_used_set().expect("a current set exists")
    }

    fn borrow_set(&mut self, index: usize) -> SmudgeSetMut<'_> {
        SmudgeSetMut {
            manager: self,
            index,
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
            items.extend(set.used_smudges().iter().cloned());
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

static THE_SMUDGE_MANAGER: OnceLock<Mutex<SmudgeManager>> = OnceLock::new();

pub fn get_smudge_manager() -> &'static Mutex<SmudgeManager> {
    THE_SMUDGE_MANAGER.get_or_init(|| Mutex::new(SmudgeManager::new()))
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
    let count = {
        let mut set = manager.borrow_set(0);
        let smudge = set.add_smudge_to_set();
        smudge.size = size;
        smudge.opacity = opacity;
        set.used_smudge_count()
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
    if manager.used_sets.is_empty() {
        return false;
    }
    let count = {
        let mut set = manager.borrow_set(0);
        if set.used_smudge_count() == 0 {
            return false;
        }
        set.remove_smudge_from_set(0);
        set.used_smudge_count()
    };
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

    fn identity(set: &SmudgeSetMut<'_>) -> *const SmudgeSet {
        std::ptr::from_ref(&**set)
    }

    #[test]
    fn remove_smudge_set_reuses_without_reset_like_cpp() {
        let mut manager = SmudgeManager::new();
        let mut set = manager.add_smudge_set();
        let original = identity(&set);
        set.add_smudge_to_set().size = 42.0;
        set.remove();
        assert_eq!(manager.free_sets.last().unwrap().used_smudge_count(), 1);
        let reused = manager.add_smudge_set();
        assert_eq!(original, identity(&reused));
        assert_eq!(reused.used_smudge_count(), 1);
        assert_eq!(reused.used_smudges()[0].size, 42.0);
    }

    #[test]
    fn collect_decal_render_items_skips_empty_and_keeps_used() {
        let mut manager = SmudgeManager::new();
        {
            let mut set = manager.add_smudge_set();
            let drawn = set.add_smudge_to_set();
            drawn.pos = Vec3::new(4.0, 5.0, 6.0);
            drawn.size = 8.0;
            drawn.opacity = 0.5;
            let skipped = set.add_smudge_to_set();
            skipped.size = 0.0;
            skipped.opacity = 1.0;
        }
        let items = manager.collect_decal_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(items[0].size, 8.0);
        assert!((items[0].color[3] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clears_used_sets_before_pooling() {
        let mut manager = SmudgeManager::new();
        let original = {
            let mut set = manager.add_smudge_set();
            set.add_smudge_to_set().size = 12.0;
            identity(&set)
        };
        manager.reset();
        let reused = manager.add_smudge_set();
        assert_eq!(original, identity(&reused));
        assert_eq!(reused.used_smudge_count(), 0);
    }

    #[test]
    fn free_smudges_follow_cpp_reuse_order_within_one_manager() {
        let mut manager = SmudgeManager::new();
        {
            let mut first = manager.add_smudge_set();
            first.add_smudge_to_set().size = 1.0;
            first.add_smudge_to_set().size = 2.0;
            first.reset();
        }
        let mut second = manager.add_smudge_set();
        assert_eq!(second.add_smudge_to_set().size, 2.0);
        assert_eq!(second.add_smudge_to_set().size, 1.0);
    }

    #[test]
    fn removing_a_smudge_keeps_the_remaining_cpp_list_order() {
        let mut manager = SmudgeManager::new();
        let mut set = manager.add_smudge_set();
        for size in [1.0, 2.0, 3.0] {
            set.add_smudge_to_set().size = size;
        }
        set.remove_smudge_from_set(1);
        assert_eq!(
            set.used_smudges()
                .iter()
                .map(|smudge| smudge.size)
                .collect::<Vec<_>>(),
            vec![1.0, 3.0]
        );
        assert_eq!(set.add_smudge_to_set().size, 2.0);
    }

    #[test]
    fn free_smudges_do_not_leak_between_managers() {
        let mut first_manager = SmudgeManager::new();
        let mut first = first_manager.add_smudge_set();
        first.add_smudge_to_set().size = 42.0;
        first.reset();
        let mut second_manager = SmudgeManager::new();
        let mut second = second_manager.add_smudge_set();
        assert_eq!(second.add_smudge_to_set().size, 0.0);
        assert_eq!(first.add_smudge_to_set().size, 42.0);
    }
}
