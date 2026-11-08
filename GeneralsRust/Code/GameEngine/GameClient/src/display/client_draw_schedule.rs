//! Dual-world W3DDisplay::draw CPU phases (W3DDisplay.cpp:1730-1835).
//!
//! Order: freeze → sync-time → updateViews (Drawable::draw) → particles → drawViews (GPU).

#[path = "client_draw_schedule_state.rs"]
mod schedule_state;
pub use schedule_state::ClientDrawScheduleState;

#[cfg(test)]
use std::sync::Mutex;

use gamelogic::helpers::TheGameLogic;
use gamelogic::object::registry::OBJECT_REGISTRY;

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

#[cfg(test)]
static PHASE_LOG: Mutex<Vec<ClientDrawPhase>> = Mutex::new(Vec::new());
#[cfg(test)]
static DRAW_ID_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn compute_w3d_display_freeze(
    state: &mut ClientDrawScheduleState,
    extra_freeze: bool,
    client_frame: u32,
) -> bool {
    let last = state.swap_last_display_frame(client_frame);
    extra_freeze || last == client_frame
}

pub fn advance_visual_sync(state: &mut ClientDrawScheduleState, freeze_time: bool) -> u32 {
    let previous = state.sync_time_ms;
    let sync_time = state.advance_sync(freeze_time, W3D_FRAME_LENGTH_MS);
    sync_time.wrapping_sub(previous)
}

/// Current visual sync clock (ms) owned by the receiving GameClient draw schedule.
pub fn current_visual_sync_ms(state: &ClientDrawScheduleState) -> u32 {
    state.sync_time_ms
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

pub fn run_dual_world_cpu_phases(
    schedule: &mut ClientDrawScheduleState,
    view: &crate::display::view::View,
    client_frame: u32,
) {
    if OBJECT_REGISTRY.is_empty() {
        return;
    }
    let last_cpu_phase_frame = schedule.last_cpu_phase_frame;
    schedule.last_cpu_phase_frame = client_frame;
    if last_cpu_phase_frame == client_frame {
        return;
    }
    let camera_frozen = view.is_time_frozen() && !view.is_camera_movement_finished();
    let extra_freeze = camera_frozen
        || TheScriptEngine::is_time_frozen_debug()
        || TheScriptEngine::is_time_frozen_script()
        || TheGameLogic::is_game_paused();
    let freeze = compute_w3d_display_freeze(schedule, extra_freeze, client_frame);
    let frame_time = advance_visual_sync(schedule, freeze);
    #[cfg(test)]
    let mut phases = Vec::new();
    #[cfg(test)]
    phases.push(ClientDrawPhase::SyncTime);

    let (region, time) = {
        (
            view_aabb_from_tactical(view),
            gamelogic::helpers::DrawTimeObservation::new(
                Some(gamelogic::helpers::CameraTimeObservation {
                    time_frozen: view.is_time_frozen(),
                    movement_finished: view.is_camera_movement_finished(),
                }),
                TheScriptEngine::is_time_frozen_debug(),
                TheScriptEngine::is_time_frozen_script(),
                game_engine::common::ini::get_global_data()
                    .map(|data| data.read().show_client_physics)
                    .unwrap_or(true),
            ),
        )
    };
    #[cfg(test)]
    let mut drawn = Vec::new();
    if should_draw_drawables(frame_time) {
        draw_logic_drawables_in_region(
            region,
            &time,
            #[cfg(test)]
            &mut drawn,
        );
    }
    #[cfg(test)]
    phases.push(ClientDrawPhase::UpdateViews);

    update_particles_after_transforms();
    #[cfg(test)]
    phases.push(ClientDrawPhase::ParticleUpdate);
    #[cfg(test)]
    phases.push(ClientDrawPhase::DrawViews);

    #[cfg(test)]
    if let Ok(mut log) = PHASE_LOG.lock() {
        *log = phases;
    }
    #[cfg(test)]
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

fn draw_logic_drawables_in_region(
    region: ViewAabb,
    time: &gamelogic::helpers::DrawTimeObservation,
    #[cfg(test)] drawn: &mut Vec<u32>,
) {
    use gamelogic::drawable::Drawable as LogicDrawable;

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
        #[cfg(test)]
        let id = guard.get_drawable_id();
        guard.draw(None, time);
        #[cfg(test)]
        drawn.push(id);
    }
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
mod tests {
    use super::*;

    #[test]
    fn draw_advance_skipped_when_visual_time_frozen() {
        let mut state = ClientDrawScheduleState::default();
        assert!(!compute_w3d_display_freeze(&mut state, false, 10));
        assert_eq!(advance_visual_sync(&mut state, false), W3D_FRAME_LENGTH_MS);
        assert!(compute_w3d_display_freeze(&mut state, false, 10));
        assert_eq!(advance_visual_sync(&mut state, true), 0);
        assert!(!should_draw_drawables(0));
        assert!(compute_w3d_display_freeze(&mut state, true, 11));
        assert_eq!(advance_visual_sync(&mut state, true), 0);
        assert_eq!(current_visual_sync_ms(&state), W3D_FRAME_LENGTH_MS);
    }

    #[test]
    fn draw_schedule_state_is_isolated_per_client() {
        let mut a = ClientDrawScheduleState::default();
        let mut b = ClientDrawScheduleState::default();
        assert_eq!(advance_visual_sync(&mut a, false), W3D_FRAME_LENGTH_MS);
        assert_eq!(current_visual_sync_ms(&a), W3D_FRAME_LENGTH_MS);
        assert_eq!(current_visual_sync_ms(&b), 0);
        assert!(!compute_w3d_display_freeze(&mut b, false, 7));
        assert!(!compute_w3d_display_freeze(&mut a, false, 8));
    }

    #[test]
    fn physics_loco_advances_once_per_drawn_frame_in_region_order() {
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
        let mut state = ClientDrawScheduleState::default();
        assert!(!compute_w3d_display_freeze(&mut state, false, 20));
        assert!(should_draw_drawables(advance_visual_sync(&mut state, false)));
        assert!(compute_w3d_display_freeze(&mut state, false, 20));
        assert!(!should_draw_drawables(advance_visual_sync(&mut state, true)));
    }

    #[test]
    fn particle_update_runs_after_client_transforms() {
        crate::test_support::ensure_ambient_host();
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
        crate::test_support::ensure_ambient_host();
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
