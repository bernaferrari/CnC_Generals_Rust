//! Explicit host-owned draw schedule regressions. Particle/GPU markers remain
//! schedule seams; the production particle driver does not consume this helper.
use super::host_draw_schedule::{
    HOST_VISUAL_FRAME_MS, HostDrawSchedule, HostPresentPhase, HostPresentVisualInput,
};
use crate::game_logic::ObjectId;
use glam::Mat4;

#[test]
fn schedules_and_cached_local_matrices_are_instance_owned() {
    let mut first = HostDrawSchedule::new();
    let mut second = HostDrawSchedule::new();
    let live = HostPresentVisualInput {
        visual_dt_ms: HOST_VISUAL_FRAME_MS,
        frozen: false,
    };
    first.begin_presented_frame(live);
    first.note_loco_applied(ObjectId(8), Mat4::from_rotation_x(0.4));
    second.begin_presented_frame(live);
    assert_eq!(second.cached_applied_matrix(ObjectId(8)), None);
    assert!(!first.should_calc_loco(ObjectId(8)));
    assert!(second.should_calc_loco(ObjectId(8)));
    assert_eq!(
        first.advance_particles_after_transforms(),
        HOST_VISUAL_FRAME_MS
    );
    assert_eq!(
        first.advance_particles_after_transforms(),
        HOST_VISUAL_FRAME_MS
    );
    assert_eq!(second.particle_visual_ms(), 0);
    first.note_gpu_phase();
    assert_eq!(
        first.phase_log(),
        vec![
            HostPresentPhase::Freeze,
            HostPresentPhase::PhysicsLoco,
            HostPresentPhase::Particles,
            HostPresentPhase::Gpu
        ]
    );
}

#[test]
fn frozen_visual_time_stops_loco_and_particle_advancement() {
    let mut state = HostDrawSchedule::new();
    state.begin_presented_frame(HostPresentVisualInput {
        visual_dt_ms: 0,
        frozen: true,
    });
    assert!(!state.should_calc_loco(ObjectId(8)));
    assert_eq!(state.advance_particles_after_transforms(), 0);
    state.note_gpu_phase();
    assert_eq!(
        state.phase_log(),
        vec![
            HostPresentPhase::Freeze,
            HostPresentPhase::Particles,
            HostPresentPhase::Gpu
        ]
    );
    state = HostDrawSchedule::default();
    assert_eq!(state.present_epoch(), 0);
    assert_eq!(state.cached_applied_matrix(ObjectId(8)), None);
}
