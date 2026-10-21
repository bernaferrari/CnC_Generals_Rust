//! Receiving camera-fade observations at the completed frame boundary.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum FrameFade {
    /// Explicit old programmatic/standalone frame API, never native publication.
    Compatibility,
    Receiving(PresentationCameraFade),
}
impl FrameFade {
    pub(super) fn sample(self) -> PresentationCameraFade {
        match self {
            Self::Receiving(fade) => fade,
            Self::Compatibility => {
                #[cfg(feature = "game_client")]
                {
                    from_overlay(game_client::display::status_circle::render_camera_fade())
                }
                #[cfg(not(feature = "game_client"))]
                {
                    PresentationCameraFade::default()
                }
            }
        }
    }
    pub(super) fn receiving(logic: &GameLogic) -> Result<Self, String> {
        let engine = logic.owned_script_engine.as_ref().ok_or(
            "Cannot freeze native camera fade: receiving interpreter is unavailable or lent",
        )?;
        let (fade, value) = engine.fade_observation();
        #[cfg(feature = "game_client")]
        let packed = from_overlay(
            game_client::display::status_circle::camera_fade_from_observation(fade, value),
        );
        #[cfg(not(feature = "game_client"))]
        let packed = {
            let _ = (fade, value);
            PresentationCameraFade::default()
        };
        Ok(Self::Receiving(packed))
    }
}
#[cfg(feature = "game_client")]
fn from_overlay(
    overlay: Option<game_client::display::status_circle::CameraFadeOverlay>,
) -> PresentationCameraFade {
    overlay
        .map(|overlay| PresentationCameraFade {
            fade: overlay.fade as u8,
            intensity: overlay.intensity,
            diffuse: overlay.diffuse,
        })
        .unwrap_or_default()
}

impl PresentationFrame {
    /// Checked native query. Validate the actual receiver before frame effects.
    pub(crate) fn try_build_from_logic(logic: &mut GameLogic, local: u32) -> Result<Self, String> {
        let fade = FrameFade::receiving(logic)?;
        Ok(Self::build_from_logic_with_tint_update(
            logic,
            local,
            None,
            true,
            Vec::new(),
            fade,
        ))
    }

    /// Checked ordinary native publication, including the existing victory path.
    /// The immutable fade input is captured before accepted-shot/tint/health drains.
    pub(crate) fn try_publish_for_engine_with_runtime_heightmap(
        logic: &mut GameLogic,
        local: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
        with_victory: bool,
    ) -> Result<Self, String> {
        let fade = FrameFade::receiving(logic)?;
        Ok(if with_victory {
            Self::build_with_victory_for_engine_with_runtime_heightmap_and_fade(
                logic,
                local,
                shadow,
                runtime_heightmap,
                fade,
            )
        } else {
            let discharges = logic.take_weapon_discharges_for_presentation();
            Self::build_for_engine_with_runtime_heightmap_and_discharges(
                logic,
                local,
                shadow,
                runtime_heightmap,
                discharges,
                fade,
            )
        })
    }
}
