//! Drawable.cpp:2527-2627: changed-look writes and the previsibility Draw phase.
use super::*;

impl BasicDrawable {
    /// The native caller supplies facts from its driving world. No object lookup
    /// or presentation query can advance this value.
    pub fn advance_material_draw(&mut self, effectively_dead: bool, frenzy: bool) {
        if !frenzy {
            self.second_material_pass_opacity = if effectively_dead {
                0.0
            } else if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity * MATERIAL_PASS_OPACITY_FADE_SCALAR
            } else {
                0.0
            };
        }
        self.overlay_data.second_material_pass_opacity = self.second_material_pass_opacity;
    }

    /// The retained Core render entry owns its own Draw invocation. Native Main
    /// enters through the explicit CPU Draw phase instead of calling render.
    pub(crate) fn advance_core_material_draw(&mut self) {
        let dead = self.object_id.is_some_and(|id| {
            OBJECT_REGISTRY
                .get_object(id)
                .and_then(|object| {
                    object
                        .read()
                        .ok()
                        .map(|object| object.is_effectively_dead())
                })
                .unwrap_or(false)
        });
        self.advance_material_draw(dead, self.test_tint_status(TintStatus::FRENZY));
    }

    pub fn material_pass_opacity(&self) -> f32 {
        self.second_material_pass_opacity
    }

    /// C++ reads these observations only on a changed look; disguise retains
    /// the residual and drives its own opacity. Caller resolves owner-specific
    /// definitions before entering this operation.
    pub fn apply_stealth_look_observed(
        &mut self,
        look: StealthLook,
        is_disguised: bool,
        is_mine: bool,
        friendly_opacity: f32,
    ) {
        self.apply_stealth_observations(look, is_disguised, is_mine, friendly_opacity, true);
    }

    /// Native logic already queued the changed-look material write at its
    /// original observation point. Refreshing presentation metadata cannot
    /// replay that write, including after a same-world visual replacement.
    pub fn apply_stealth_look_metadata(
        &mut self,
        look: StealthLook,
        is_disguised: bool,
        is_mine: bool,
        friendly_opacity: f32,
    ) {
        self.apply_stealth_observations(look, is_disguised, is_mine, friendly_opacity, false);
    }

    fn apply_stealth_observations(
        &mut self,
        look: StealthLook,
        is_disguised: bool,
        is_mine: bool,
        friendly_opacity: f32,
        write_material: bool,
    ) {
        if look == self.stealth_look {
            return;
        }
        self.stealth_opacity = 1.0;
        self.hidden_by_stealth = look == StealthLook::Invisible;
        match look {
            StealthLook::VisibleFriendly | StealthLook::VisibleFriendlyDetected => {
                if !is_disguised {
                    self.stealth_opacity = friendly_opacity;
                    if write_material {
                        self.second_material_pass_opacity =
                            if look == StealthLook::VisibleFriendlyDetected && !is_mine {
                                1.0
                            } else {
                                0.0
                            };
                    }
                }
            }
            StealthLook::VisibleDetected => {
                if write_material {
                    self.second_material_pass_opacity = if is_mine { 0.0 } else { 1.0 };
                }
            }
            StealthLook::None | StealthLook::DisguisedEnemy | StealthLook::Invisible => {
                if write_material {
                    self.second_material_pass_opacity = 0.0;
                }
            }
        }
        self.stealth_look = look;
        self.update_hidden_status();
        self.overlay_data.second_material_pass_opacity = self.second_material_pass_opacity;
    }
}
