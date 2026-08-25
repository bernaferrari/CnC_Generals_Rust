//! Host present-path schedule: one loco step per presented frame.

use super::host_draw_schedule::{
    HOST_VISUAL_FRAME_MS, HostPresentPhase, HostPresentVisualInput, begin_presented_frame,
    particle_visual_ms, phase_log, reset_host_present_schedule, run_host_present_visual_phases,
};
use super::physics_visual_host::{
    HostPhysicsVisualFacts, apply_to_world_matrix, insert_facts_for_test, loco_state,
    reset_host_physics_visual_state,
};
use crate::game_logic::ObjectId;
use game_client::physics_visual::{
    LocomotorVisualParams, PhysicsVisualAppearance, PhysicsVisualBody, PhysicsVisualLocoState,
};
use glam::Mat4;

fn hover_facts(frozen: bool) -> HostPhysicsVisualFacts {
    HostPhysicsVisualFacts {
        appearance: PhysicsVisualAppearance::Hover,
        params: LocomotorVisualParams {
            forward_accel_coef: 0.5,
            pitch_stiffness: 0.1,
            pitch_damping: 0.9,
            uniform_axial_damping: 1.0,
            ..LocomotorVisualParams::default()
        },
        body: PhysicsVisualBody {
            is_motive: true,
            accel_x: 4.0,
            dir_x: 1.0,
            dir_y: 0.0,
            ..PhysicsVisualBody::default()
        },
        object_disabled_held: false,
        show_client_physics: true,
        tactical_view_time_frozen: false,
        camera_movement_finished: true,
        script_time_frozen_debug: false,
        script_time_frozen_script: frozen,
    }
}

fn live_present() -> HostPresentVisualInput {
    HostPresentVisualInput {
        visual_dt_ms: HOST_VISUAL_FRAME_MS,
        frozen: false,
    }
}

#[test]
fn loco_advances_once_per_presented_frame_even_with_two_presents_per_logic_frame() {
    reset_host_physics_visual_state();
    let id = ObjectId(801);
    insert_facts_for_test(id, hover_facts(false));

    begin_presented_frame(live_present());
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    let after_first_present = loco_state(id);
    assert_ne!(after_first_present, PhysicsVisualLocoState::default());

    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    assert_eq!(
        loco_state(id),
        after_first_present,
        "second world_matrix in the same present must not step loco"
    );

    begin_presented_frame(live_present());
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    let after_second_present = loco_state(id);
    assert_ne!(
        after_second_present, after_first_present,
        "a second present of the same logic frame still steps loco once"
    );
}

#[test]
fn frozen_visual_time_stops_loco_and_particle_advancement() {
    reset_host_physics_visual_state();
    let id = ObjectId(802);
    insert_facts_for_test(id, hover_facts(false));

    let phases = run_host_present_visual_phases(
        HostPresentVisualInput {
            visual_dt_ms: 0,
            frozen: true,
        },
        || {
            let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
        },
    );
    assert_eq!(loco_state(id), PhysicsVisualLocoState::default());
    assert_eq!(particle_visual_ms(), 0);
    assert_eq!(
        phases,
        vec![
            HostPresentPhase::Freeze,
            HostPresentPhase::Particles,
            HostPresentPhase::Gpu,
        ]
    );
}

#[test]
fn particles_advance_after_transforms() {
    reset_host_physics_visual_state();
    let id = ObjectId(803);
    insert_facts_for_test(id, hover_facts(false));

    reset_host_present_schedule();
    begin_presented_frame(live_present());
    assert_eq!(particle_visual_ms(), 0);
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    assert_eq!(
        particle_visual_ms(),
        0,
        "particle visual time stays put until after transforms"
    );
    let ms = super::host_draw_schedule::advance_particles_after_transforms();
    assert_eq!(ms, HOST_VISUAL_FRAME_MS);
    super::host_draw_schedule::note_gpu_phase();
    assert_eq!(
        phase_log(),
        vec![
            HostPresentPhase::Freeze,
            HostPresentPhase::PhysicsLoco,
            HostPresentPhase::Particles,
            HostPresentPhase::Gpu,
        ]
    );
}
