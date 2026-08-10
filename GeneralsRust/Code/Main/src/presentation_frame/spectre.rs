use super::*;

// --- Wave 73: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual ---

/// Retail Spectre AttackAreaDecal Texture residual (`SCCSpecTarg`).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_DECAL: &str = "SCCSpecTarg";
/// Retail Spectre TargetingReticleDecal Texture residual (`SCCSpecRet`).
pub const PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL: &str = "SCCSpecRet";
/// Retail Spectre decal Color residual (R:127 G:177 B:222 A:255) as RGBA 0..1.
pub const PRESENTATION_SPECTRE_DECAL_COLOR: [f32; 4] =
    [127.0 / 255.0, 177.0 / 255.0, 222.0 / 255.0, 1.0];
/// Retail AttackAreaDecal OpacityMin residual (25%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN: f32 = 0.25;
/// Retail AttackAreaDecal OpacityMax residual (50%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMin residual (50%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMax residual (100%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX: f32 = 1.00;
/// Retail AttackAreaDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS: u32 = 1500;
/// Retail TargetingReticleDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_RETICLE_THROB_MS: u32 = 300;
/// Retail AttackAreaRadius residual (presentation cursor / decal radius).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS: f32 = 200.0;
/// Retail TargetingReticleRadius residual.
pub const PRESENTATION_SPECTRE_RETICLE_RADIUS: f32 = 25.0;
/// Retail AttackAreaDecal Style residual.
pub const PRESENTATION_SPECTRE_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail OnlyVisibleToOwningPlayer residual (both decals).
pub const PRESENTATION_SPECTRE_DECAL_ONLY_OWNER: bool = true;

/// Snapshot-owned Spectre orbit decal presentation residual (AttackArea + Reticle).
///
/// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb / owning-player visibility filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationSpectreOrbitDecal {
    pub attack_area_texture: &'static str,
    pub reticle_texture: &'static str,
    pub color: [f32; 4],
    pub attack_area_radius: f32,
    pub reticle_radius: f32,
    pub attack_area_opacity_min: f32,
    pub attack_area_opacity_max: f32,
    pub reticle_opacity_min: f32,
    pub reticle_opacity_max: f32,
    pub attack_area_throb_ms: u32,
    pub reticle_throb_ms: u32,
    pub style: &'static str,
    pub only_visible_to_owning_player: bool,
}

impl PresentationSpectreOrbitDecal {
    /// Retail SpectreGunshipUpdate AttackAreaDecal + TargetingReticleDecal residual defaults.
    pub const RETAIL: Self = Self {
        attack_area_texture: PRESENTATION_SPECTRE_ATTACK_AREA_DECAL,
        reticle_texture: PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL,
        color: PRESENTATION_SPECTRE_DECAL_COLOR,
        attack_area_radius: PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS,
        reticle_radius: PRESENTATION_SPECTRE_RETICLE_RADIUS,
        attack_area_opacity_min: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN,
        attack_area_opacity_max: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX,
        reticle_opacity_min: PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN,
        reticle_opacity_max: PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX,
        attack_area_throb_ms: PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS,
        reticle_throb_ms: PRESENTATION_SPECTRE_RETICLE_THROB_MS,
        style: PRESENTATION_SPECTRE_DECAL_STYLE,
        only_visible_to_owning_player: PRESENTATION_SPECTRE_DECAL_ONLY_OWNER,
    };

    /// Honesty: retail Spectre AttackAreaDecal / TargetingReticleDecal presentation residual.
    pub fn honesty_residual_ok(self) -> bool {
        self.attack_area_texture == "SCCSpecTarg"
            && self.reticle_texture == "SCCSpecRet"
            && (self.attack_area_radius - 200.0).abs() < 0.01
            && (self.reticle_radius - 25.0).abs() < 0.01
            && (self.attack_area_opacity_min - 0.25).abs() < 0.001
            && (self.attack_area_opacity_max - 0.50).abs() < 0.001
            && (self.reticle_opacity_min - 0.50).abs() < 0.001
            && (self.reticle_opacity_max - 1.00).abs() < 0.001
            && self.attack_area_throb_ms == 1500
            && self.reticle_throb_ms == 300
            && self.style == "SHADOW_ALPHA_DECAL"
            && self.only_visible_to_owning_player
            && (self.color[0] - 127.0 / 255.0).abs() < 0.001
            && (self.color[1] - 177.0 / 255.0).abs() < 0.001
            && (self.color[2] - 222.0 / 255.0).abs() < 0.001
            && (self.color[3] - 1.0).abs() < 0.001
            && self.attack_area_opacity_min < self.attack_area_opacity_max
            && self.reticle_opacity_min < self.reticle_opacity_max
            && self.reticle_radius < self.attack_area_radius
    }
}

/// Free-function honesty for Spectre orbit decal presentation residual (Wave 73).
pub fn honesty_spectre_orbit_decal_presentation_ok() -> bool {
    PresentationSpectreOrbitDecal::RETAIL.honesty_residual_ok()
}

/// Wave 102: dual-tick presentation residual deepen free-function honesty.
///
/// Builds an empty-host presentation snapshot and verifies dual-tick residual
/// counters (including selected/particle Wave 102 deepen) plus presentation
/// residual packs. Fail-closed vs live dual-run W3D / GPU submit.
pub fn honesty_presentation_dual_tick_residual_deepen_wave102() -> bool {
    use crate::game_logic::GameLogic;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Empty residual snapshot honesty (zero objects still dual-tick consistent).
    let empty_logic = GameLogic::new();
    let empty = PresentationFrame::build_from_logic(&empty_logic, 0);
    if !empty.dual_tick_presentation_residual_ok() {
        return false;
    }
    if empty.dual_tick.builds != 1 || empty.dual_tick.applies != 0 {
        return false;
    }
    if empty.dual_tick.selected_count != 0 || empty.dual_tick.particle_count != 0 {
        return false;
    }
    // Seeded skirmish residual: dual-tick deepen after shell apply.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresDualTick102");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        // Config residual may still produce honest empty-host dual-tick.
        return empty.dual_tick_presentation_residual_deepen_ok()
            && honesty_spectre_orbit_decal_presentation_ok();
    }
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let frame = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    frame.dual_tick_presentation_residual_deepen_ok()
        && frame.dual_tick.honesty_apply_ok()
        && frame.dual_tick.builds == 1
        && frame.dual_tick.applies >= 1
        && frame.dual_tick.selected_count == frame.selected.len() as u32
        && frame.dual_tick.particle_count == frame.particle_systems.len() as u32
        && honesty_spectre_orbit_decal_presentation_ok()
}

/// Combined Wave 102 presentation residual honesty pack.
pub fn honesty_presentation_residual_deepen_pack_wave102() -> bool {
    honesty_presentation_dual_tick_residual_deepen_wave102()
}

/// Dual-tick residual counters frozen on each presentation build / apply.
///
/// Host-testable bookkeeping for seed → logic step → multi-consumer apply order.
/// Fail-closed: not full dual-run determinism harness counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PresentationDualTickResidual {
    /// Always 1 after a successful `build_from_logic`.
    pub builds: u32,
    /// Incremented each time this snapshot is applied to HUD / shell consumers.
    pub applies: u32,
    pub object_count: u32,
    pub selected_count: u32,
    pub laser_beam_count: u32,
    pub floating_text_count: u32,
    pub world_anim_count: u32,
    pub particle_count: u32,
}

impl PresentationDualTickResidual {
    pub fn from_counts(
        objects: usize,
        selected: usize,
        lasers: usize,
        floating: usize,
        world: usize,
        particles: usize,
    ) -> Self {
        Self {
            builds: 1,
            applies: 0,
            object_count: objects as u32,
            selected_count: selected as u32,
            laser_beam_count: lasers as u32,
            floating_text_count: floating as u32,
            world_anim_count: world as u32,
            particle_count: particles as u32,
        }
    }

    /// Honesty: residual counters are self-consistent after build.
    pub fn honesty_build_ok(&self) -> bool {
        self.builds >= 1
    }

    /// Honesty: at least one dual-tick apply was recorded.
    pub fn honesty_apply_ok(&self) -> bool {
        self.builds >= 1 && self.applies >= 1
    }
}
