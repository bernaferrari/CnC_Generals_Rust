use super::*;
use game_client::core::subsystems::InGameUISubsystem;
use game_client::gui::game_window::WindowStatus;
use game_engine::common::system::radar::RadarEventType;

fn ui() -> InGameUISubsystem {
    InGameUISubsystem::default()
}
fn owner() -> RadarSystem {
    let mut radar = RadarSystem::new();
    radar.new_map_owned(Coord3D::new(0., 0., 0.), Coord3D::new(128., 128., 0.), &[]);
    radar.force_on(true);
    radar.create_event(&Coord3D::new(64., 64., 0.), RadarEventType::BeaconPulse, 1.);
    radar
}
fn windows() -> WindowManager {
    let mut windows = WindowManager::new();
    let window = windows.create_window(None, 0, 0, 100, 100).unwrap();
    window.borrow_mut().set_status(WindowStatus::ENABLED);
    window.borrow_mut().set_radar_draw_callback();
    windows
}
fn camera() -> RadarCamera {
    RadarCamera::capture(&crate::presentation_frame::SessionView::default(), 0.)
}
#[test]
fn native_radar_draw_admits_only_receiving_ui_once() {
    let owner = owner();
    let peer = self::owner();
    let mut ui = ui();
    let mut peer_ui = self::ui();
    ui.set_radar_presentation(Arc::new(owner.presentation()));
    peer_ui.set_radar_presentation(Arc::new(peer.presentation()));
    let frame = NativeRadarFrame::capture(camera(), &owner, Some(&ui));
    let mut windows = windows();
    frame.draw_windows(&mut windows, &mut Default::default(), Some(&mut ui));
    assert_eq!(ui.take_radar_acknowledgments().len(), 1);
    assert!(peer_ui.take_radar_acknowledgments().is_empty());
    frame.draw_windows(&mut windows, &mut Default::default(), Some(&mut ui));
    assert!(ui.take_radar_acknowledgments().is_empty());
}
#[test]
fn native_radar_rejects_foreign_and_previous_map_presentations() {
    let mut owner = owner();
    let peer = self::owner();
    let mut ui = ui();
    ui.set_radar_presentation(Arc::new(peer.presentation()));
    let foreign = NativeRadarFrame::capture(camera(), &owner, Some(&ui));
    assert!(foreign.radar.is_none());
    foreign.draw_windows(&mut windows(), &mut Default::default(), Some(&mut ui));
    assert!(ui.take_radar_acknowledgments().is_empty());
    let mut ui = self::ui();
    ui.set_radar_presentation(Arc::new(owner.presentation()));
    owner.new_map_owned(Coord3D::new(0., 0., 0.), Coord3D::new(256., 256., 0.), &[]);
    let stale = NativeRadarFrame::capture(camera(), &owner, Some(&ui));
    assert!(stale.radar.is_none());
    stale.draw_windows(&mut windows(), &mut Default::default(), Some(&mut ui));
    assert!(ui.take_radar_acknowledgments().is_empty());
}
#[test]
fn native_radar_absent_ui_is_explicitly_empty() {
    let frame = NativeRadarFrame::capture(camera(), &owner(), None);
    assert!(frame.radar.is_none());
    frame.draw_windows(&mut windows(), &mut Default::default(), None);
}
