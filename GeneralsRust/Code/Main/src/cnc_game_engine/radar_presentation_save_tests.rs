//! C++ W3DRadar::drawEvents soundPlayed admission before Radar::xfer.
use super::admit_radar_presentations;
use crate::game_logic::GameLogic;
use crate::save_load::snapshot::persist_v18::capture_persist_v18;
use game_client::core::subsystems::{InGameUISubsystem, InGameUiHandle};
use game_client::helpers::InGameUiHooks;
use game_engine::common::system::radar::{Coord3D, RadarEventType};
use std::sync::{Arc, Mutex};

#[test]
fn radar_draw_receipt_is_admitted_before_save_capture_without_next_tick() {
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    let position = Coord3D::new(0.5, 0.5, 0.0);
    first
        .radar_mut()
        .create_event(&position, RadarEventType::Information, 1.0);
    second
        .radar_mut()
        .create_event(&position, RadarEventType::Information, 1.0);
    let handle = Arc::new(InGameUiHandle::new(Arc::new(Mutex::new(
        InGameUISubsystem::default(),
    ))));
    let other_handle = InGameUiHandle::new(Arc::new(Mutex::new(InGameUISubsystem::default())));
    let other_frozen = Arc::new(second.radar().presentation());
    other_handle.set_radar_presentation(other_frozen.clone());
    assert_eq!(other_handle.admit_radar_draw(&other_frozen), 1);
    let frozen = Arc::new(first.radar().presentation());
    handle.set_radar_presentation(frozen.clone());
    assert_eq!(handle.admit_radar_draw(&frozen), 1);
    assert!(!capture_persist_v18(&first).radar_events[0].sound_played);

    // The native Save boundary calls this same function before capturing any state.
    admit_radar_presentations(&mut first, handle.take_radar_acknowledgments());
    assert!(capture_persist_v18(&first).radar_events[0].sound_played);
    assert!(!capture_persist_v18(&second).radar_events[0].sound_played);
    let other_receipts = other_handle.take_radar_acknowledgments();
    assert_eq!(other_receipts.len(), 1, "A admission must not drain B's UI");
    admit_radar_presentations(&mut second, other_receipts);
    assert!(capture_persist_v18(&second).radar_events[0].sound_played);
    assert_eq!(first.frame, 0, "save admission does not advance simulation");
    assert_eq!(handle.admit_radar_draw(&frozen), 0, "repeat draw is silent");
}
