//! Immutable camera/script observations for one receiving engine operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraTimeObservation {
    pub time_frozen: bool,
    pub movement_finished: bool,
}
impl CameraTimeObservation {
    pub const fn freezes_time(self) -> bool {
        self.time_frozen && !self.movement_finished
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawTimeObservation {
    pub camera: Option<CameraTimeObservation>,
    pub script_debug_frozen: bool,
    pub script_frozen: bool,
    pub show_client_physics: bool,
}
impl DrawTimeObservation {
    pub const fn new(
        camera: Option<CameraTimeObservation>,
        script_debug_frozen: bool,
        script_frozen: bool,
        show_client_physics: bool,
    ) -> Self {
        Self {
            camera,
            script_debug_frozen,
            script_frozen,
            show_client_physics,
        }
    }
    pub fn is_frozen(self) -> bool {
        self.camera.is_some_and(CameraTimeObservation::freezes_time)
            || self.script_debug_frozen
            || self.script_frozen
    }
    pub fn client_physics_active(self) -> bool {
        self.show_client_physics && !self.is_frozen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_receiving_observations_preserve_camera_debug_script_and_physics_terms() {
        for camera_frozen in [false, true] {
            for movement_finished in [false, true] {
                for debug in [false, true] {
                    for script in [false, true] {
                        for show in [false, true] {
                            let input = DrawTimeObservation::new(
                                Some(CameraTimeObservation {
                                    time_frozen: camera_frozen,
                                    movement_finished,
                                }),
                                debug,
                                script,
                                show,
                            );
                            let expected = (camera_frozen && !movement_finished) || debug || script;
                            assert_eq!(input.is_frozen(), expected);
                            assert_eq!(input.client_physics_active(), show && !expected);
                        }
                    }
                }
            }
        }
        assert!(!DrawTimeObservation::new(None, false, false, true).is_frozen());
        let peer = DrawTimeObservation::new(
            Some(CameraTimeObservation {
                time_frozen: true,
                movement_finished: false,
            }),
            false,
            false,
            true,
        );
        let running = DrawTimeObservation::new(
            Some(CameraTimeObservation {
                time_frozen: true,
                movement_finished: true,
            }),
            false,
            false,
            true,
        );
        assert!(peer.is_frozen());
        assert!(!running.is_frozen());
        assert!(peer.is_frozen());
    }
}
