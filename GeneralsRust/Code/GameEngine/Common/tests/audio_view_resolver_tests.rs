#![cfg(feature = "internal")]

use game_engine::common::audio::audio_event_rts::Coord3D;
use game_engine::common::audio::game_audio::{
    register_audio_view_resolver, AudioManager, AudioViewResolver, Real,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct TestAudioViewResolver {
    tactical_view_position: Coord3D,
    tactical_view_angle: Real,
    camera_position: Coord3D,
    ground_height: Real,
    calls: AtomicUsize,
}

impl TestAudioViewResolver {
    fn new(
        tactical_view_position: Coord3D,
        tactical_view_angle: Real,
        camera_position: Coord3D,
        ground_height: Real,
    ) -> Self {
        Self {
            tactical_view_position,
            tactical_view_angle,
            camera_position,
            ground_height,
            calls: AtomicUsize::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AudioViewResolver for TestAudioViewResolver {
    fn get_tactical_view_position(&self) -> Coord3D {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.tactical_view_position
    }

    fn get_tactical_view_angle(&self) -> Real {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.tactical_view_angle
    }

    fn get_3d_camera_position(&self) -> Coord3D {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.camera_position
    }

    fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ground_height
    }
}

#[test]
fn audio_update_consumes_registered_view_resolver() {
    let resolver = Arc::new(TestAudioViewResolver::new(
        Coord3D {
            x: 12.0,
            y: 34.0,
            z: 56.0,
        },
        0.0,
        Coord3D {
            x: 12.0,
            y: 34.0,
            z: 1056.0,
        },
        16.0,
    ));

    assert!(register_audio_view_resolver(resolver.clone()));

    let mut manager = AudioManager::new();
    manager.update();

    let listener = manager.get_listener_position();
    assert_eq!(listener.x, 12.0);
    assert_eq!(listener.y, 34.0);
    assert_eq!(listener.z, 216.0);
    assert!((manager.get_zoom_volume() - 0.911_111_1).abs() < 1.0e-5);
    assert_eq!(resolver.call_count(), 4);
}
