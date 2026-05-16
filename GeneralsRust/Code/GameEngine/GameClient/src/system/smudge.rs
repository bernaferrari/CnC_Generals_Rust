//! Smudge system (terrain decals), matching System/Smudge.cpp.

use glam::{Vec2, Vec3};
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

    pub fn remove_smudge_set(&mut self, set: &SmudgeSetHandle) {
        if let Some(pos) = self
            .used_sets
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, set))
        {
            let set = self.used_sets.swap_remove(pos);
            if let Ok(mut guard) = set.lock() {
                guard.reset();
            }
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
