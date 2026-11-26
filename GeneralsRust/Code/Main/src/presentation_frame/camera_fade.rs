//! Sample the receiving interpreter's completed camera-fade state.
use super::{GameLogic, PresentationCameraFade};

pub(super) fn freeze_camera_fade(logic: &GameLogic) -> Result<PresentationCameraFade, String> {
    let engine = logic
        .owned_script_engine
        .as_ref()
        .ok_or("Cannot freeze native camera fade: receiving interpreter is unavailable or lent")?;
    let (fade, value) = engine.fade_observation();
    #[cfg(feature = "game_client")]
    let camera_fade =
        game_client::display::status_circle::camera_fade_from_observation(fade, value)
            .map(|overlay| PresentationCameraFade {
                fade: overlay.fade as u8,
                intensity: overlay.intensity,
                diffuse: overlay.diffuse,
            })
            .unwrap_or_default();
    #[cfg(not(feature = "game_client"))]
    let camera_fade = {
        let _ = (fade, value);
        PresentationCameraFade::default()
    };
    Ok(camera_fade)
}
