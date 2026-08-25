//! Dual-world W3DDisplay::draw CPU phases (W3DDisplay.cpp:1730-1835).
//!
//! Order: freeze → sync-time → updateViews (Drawable::draw) → particles → drawViews (GPU).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use gamelogic::helpers::TheGameLogic;
use gamelogic::object::registry::OBJECT_REGISTRY;
use ww3d_core::WW3D;

use crate::core::game_client::live_game_client_frame;
use crate::display::view::with_tactical_view_ref;
use crate::effects::particle_manager::get_particle_system_manager_mut;
use gamelogic::helpers::TheScriptEngine;

const W3D_FRAME_LENGTH_MS: u32 = 33;
const DRAWABLE_OVERSCAN: f32 = 75.0;
const VIEW_REGION_Y_PAD: f32 = 60.0;
const SAFE_Z: f32 = 999_999.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientDrawPhase {
    SyncTime,
    UpdateViews,
    ParticleUpdate,
    DrawViews,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewAabb {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

impl ViewAabb {
    pub fn contains(self, pos: [f32; 3]) -> bool {
        pos[0] >= self.lo[0]
            && pos[0] <= self.hi[0]
            && pos[1] >= self.lo[1]
            && pos[1] <= self.hi[1]
            && pos[2] >= self.lo[2]
            && pos[2] <= self.hi[2]
    }

    pub fn unbounded() -> Self {
        Self {
            lo: [f32::MIN, f32::MIN, f32::MIN],
            hi: [f32::MAX, f32::MAX, f32::MAX],
        }
    }
}

static SYNC_TIME_MS: AtomicU32 = AtomicU32::new(0);
static LAST_DISPLAY_CLIENT_FRAME: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_CPU_PHASE_FRAME: AtomicU32 = AtomicU32::new(u32::MAX);
static PHASE_LOG: Mutex<Vec<ClientDrawPhase>> = Mutex::new(Vec::new());
static DRAW_ID_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn extra_freeze_from_engine() -> bool {
    let camera_frozen =
        with_tactical_view_ref(|view| view.is_time_frozen() && !view.is_camera_movement_finished());
    camera_frozen
        || TheScriptEngine::is_time_frozen_debug()
        || TheScriptEngine::is_time_frozen_script()
        || TheGameLogic::is_game_paused()
}

pub fn compute_w3d_display_freeze(extra_freeze: bool, client_frame: u32) -> bool {
    let last = LAST_DISPLAY_CLIENT_FRAME.swap(client_frame, Ordering::SeqCst);
    extra_freeze || last == client_frame
}

pub fn advance_visual_sync(freeze_time: bool) -> u32 {
    if !freeze_time {
        let _ = SYNC_TIME_MS.fetch_add(W3D_FRAME_LENGTH_MS, Ordering::SeqCst);
    }
    let sync_time = SYNC_TIME_MS.load(Ordering::SeqCst);
    WW3D::sync(sync_time);
    WW3D::sync_time().saturating_sub(WW3D::previous_sync_time())
}

pub fn should_draw_drawables(frame_time_ms: u32) -> bool {
    frame_time_ms != 0
}

pub fn run_scheduled_phases<Draw, Particles>(
    frame_time_ms: u32,
    mut update_views: Draw,
    mut particles: Particles,
) -> Vec<ClientDrawPhase>
where
    Draw: FnMut(),
    Particles: FnMut(),
{
    let mut phases = vec![ClientDrawPhase::SyncTime];
    update_views();
    phases.push(ClientDrawPhase::UpdateViews);
    let _ = frame_time_ms;
    particles();
    phases.push(ClientDrawPhase::ParticleUpdate);
    phases.push(ClientDrawPhase::DrawViews);
    phases
}

pub fn drawables_in_region_order(ids_and_pos: &[(u32, [f32; 3])], region: ViewAabb) -> Vec<u32> {
    ids_and_pos
        .iter()
        .filter(|(_, pos)| region.contains(*pos))
        .map(|(id, _)| *id)
        .collect()
}

pub fn run_dual_world_cpu_phases() {
    if OBJECT_REGISTRY.is_empty() {
        return;
    }
    let client_frame = live_game_client_frame().unwrap_or_else(TheGameLogic::get_frame);
    if LAST_CPU_PHASE_FRAME.swap(client_frame, Ordering::SeqCst) == client_frame {
        return;
    }
    let freeze = compute_w3d_display_freeze(extra_freeze_from_engine(), client_frame);
    let frame_time = advance_visual_sync(freeze);
    let mut phases = Vec::new();
    phases.push(ClientDrawPhase::SyncTime);

    let region = with_tactical_view_ref(view_aabb_from_tactical);
    let drawn = if should_draw_drawables(frame_time) {
        draw_logic_drawables_in_region(region)
    } else {
        Vec::new()
    };
    phases.push(ClientDrawPhase::UpdateViews);

    update_particles_after_transforms();
    phases.push(ClientDrawPhase::ParticleUpdate);
    phases.push(ClientDrawPhase::DrawViews);

    if let Ok(mut log) = PHASE_LOG.lock() {
        *log = phases;
    }
    if let Ok(mut log) = DRAW_ID_LOG.lock() {
        *log = drawn;
    }
}

fn update_particles_after_transforms() {
    if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
        if let Some(manager) = manager_guard.as_mut() {
            let frame = TheGameLogic::get_frame();
            manager.update(0, frame);
        }
    }
}

fn draw_logic_drawables_in_region(region: ViewAabb) -> Vec<u32> {
    use gamelogic::drawable::Drawable as LogicDrawable;

    let mut drawn = Vec::new();
    for object in OBJECT_REGISTRY.get_all_objects() {
        let Some(drawable) = object.read().ok().and_then(|obj| obj.get_drawable()) else {
            continue;
        };
        let Ok(mut guard) = drawable.write() else {
            continue;
        };
        let pos = guard.get_position();
        if !region.contains([pos.x, pos.y, pos.z]) {
            continue;
        }
        let id = guard.get_drawable_id();
        guard.draw(None);
        drawn.push(id);
    }
    drawn
}

fn view_aabb_from_tactical(view: &crate::display::view::View) -> ViewAabb {
    let Ok(box_pts) = view.get_screen_corner_world_points_at_z(0.0) else {
        return ViewAabb::unbounded();
    };
    let mut lo_x = box_pts[0].x;
    let mut lo_y = box_pts[0].y;
    let mut hi_x = box_pts[0].x;
    let mut hi_y = box_pts[0].y;
    for pt in &box_pts {
        lo_x = lo_x.min(pt.x);
        lo_y = lo_y.min(pt.y);
        hi_x = hi_x.max(pt.x);
        hi_y = hi_y.max(pt.y);
    }
    let bias = view.guard_band_bias();
    lo_x -= DRAWABLE_OVERSCAN + bias.x;
    lo_y -= DRAWABLE_OVERSCAN + bias.y + VIEW_REGION_Y_PAD;
    hi_x += DRAWABLE_OVERSCAN + bias.x;
    hi_y += DRAWABLE_OVERSCAN + bias.y;
    ViewAabb {
        lo: [lo_x, lo_y, -SAFE_Z],
        hi: [hi_x, hi_y, SAFE_Z],
    }
}

#[cfg(test)]
pub fn take_phase_log() -> Vec<ClientDrawPhase> {
    PHASE_LOG
        .lock()
        .map(|mut log| std::mem::take(&mut *log))
        .unwrap_or_default()
}

#[cfg(test)]
pub fn take_draw_id_log() -> Vec<u32> {
    DRAW_ID_LOG
        .lock()
        .map(|mut log| std::mem::take(&mut *log))
        .unwrap_or_default()
}

#[cfg(test)]
pub fn reset_visual_sync_for_test() {
    SYNC_TIME_MS.store(0, Ordering::SeqCst);
    LAST_DISPLAY_CLIENT_FRAME.store(u32::MAX, Ordering::SeqCst);
    LAST_CPU_PHASE_FRAME.store(u32::MAX, Ordering::SeqCst);
    WW3D::sync(0);
    WW3D::sync(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_advance_skipped_when_visual_time_frozen() {
        reset_visual_sync_for_test();
        assert!(!compute_w3d_display_freeze(false, 10));
        assert_eq!(advance_visual_sync(false), W3D_FRAME_LENGTH_MS);
        assert!(compute_w3d_display_freeze(false, 10));
        assert_eq!(advance_visual_sync(true), 0);
        assert!(!should_draw_drawables(0));
        assert!(compute_w3d_display_freeze(true, 11));
        assert_eq!(advance_visual_sync(true), 0);
    }

    #[test]
    fn physics_loco_advances_once_per_drawn_frame_in_region_order() {
        reset_visual_sync_for_test();
        let region = ViewAabb {
            lo: [0.0, 0.0, -10.0],
            hi: [10.0, 10.0, 10.0],
        };
        let candidates = [
            (3, [1.0, 1.0, 0.0]),
            (1, [20.0, 0.0, 0.0]),
            (2, [2.0, 2.0, 0.0]),
        ];
        let first = drawables_in_region_order(&candidates, region);
        assert_eq!(first, vec![3, 2]);
        let second = drawables_in_region_order(&candidates, region);
        assert_eq!(second, first);
        assert!(!compute_w3d_display_freeze(false, 20));
        assert!(should_draw_drawables(advance_visual_sync(false)));
        assert!(compute_w3d_display_freeze(false, 20));
        assert!(!should_draw_drawables(advance_visual_sync(true)));
    }

    #[test]
    fn particle_update_runs_after_client_transforms() {
        use std::cell::RefCell;
        let order = RefCell::new(Vec::new());
        let phases = run_scheduled_phases(
            W3D_FRAME_LENGTH_MS,
            || order.borrow_mut().push("updateViews"),
            || order.borrow_mut().push("particles"),
        );
        assert_eq!(
            phases,
            [
                ClientDrawPhase::SyncTime,
                ClientDrawPhase::UpdateViews,
                ClientDrawPhase::ParticleUpdate,
                ClientDrawPhase::DrawViews,
            ]
        );
        assert_eq!(*order.borrow(), ["updateViews", "particles"]);
    }

    #[test]
    fn rider_nested_draw_reenters_drawable_draw() {
        let mut draws = Vec::new();
        let phases = run_scheduled_phases(
            W3D_FRAME_LENGTH_MS,
            || {
                draws.push("parent");
                draws.push("rider");
            },
            || {},
        );
        assert_eq!(draws, ["parent", "rider"]);
        assert_eq!(phases[1], ClientDrawPhase::UpdateViews);
    }
}
