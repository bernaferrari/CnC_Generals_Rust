use super::*;
use crate::fow_rendering::ProjectedShroudSnapshot;

impl PresentationFrame {
    /// Lookup snapshot FOW for an object (local player). None if not on the frame.
    pub fn fow_for_object(&self, id: ObjectId) -> Option<ObjectVisibility> {
        self.objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility)
    }

    /// Local-player cell-grid FOW frozen on this frame (terrain / minimap).
    #[inline]
    pub fn fow_grid(&self) -> &PresentationFowGrid {
        &self.fow_grid
    }

    /// Source-shaped W3D shroud projection input frozen for this presentation
    /// frame.  This is the only shroud texture source that renderer material
    /// passes may consume.
    #[inline]
    pub fn projected_shroud(&self) -> &ProjectedShroudSnapshot {
        &self.projected_shroud
    }

    /// Projection input eligible for terrain/object shroud material passes.
    /// Shell maps and inactive/corrupt snapshots fail open exactly like the
    /// existing terrain overlay path; callers must not retain a prior texture.
    #[inline]
    pub fn terrain_projected_shroud(&self) -> Option<&ProjectedShroudSnapshot> {
        (self.terrain_fow_overlay_active() && self.projected_shroud.is_uploadable())
            .then_some(&self.projected_shroud)
    }

    /// R8 terrain FOW texture from the snapshot only (no live shroud lock).
    ///
    /// Returns `None` when the grid is inactive (fail-open: skip overlay upload)
    /// or when shell bypass already forces fully visible cells that need no darkening.
    /// Callers that always want bytes can use `fow_grid().to_r8_texture()` directly.
    pub fn terrain_fow_r8(&self) -> Option<Vec<u8>> {
        if !self.fow_grid.active {
            return None;
        }
        let r8 = self.fow_grid.to_r8_texture();
        if r8.is_empty() { None } else { Some(r8) }
    }

    /// True when terrain FOW overlay should darken from the presentation grid.
    ///
    /// Shell bypass and inactive grids are fail-open (no overlay).
    pub fn terrain_fow_overlay_active(&self) -> bool {
        self.fow_grid.active && !self.fow_shell_bypass
    }

    /// All alive presentation objects including engine-bridged (for FOW/id lists).
    pub fn alive_renderables(&self) -> impl Iterator<Item = &RenderableObject> {
        // Wave 1108: alive residual excludes sold.
        self.objects.iter().filter(|o| !o.destroyed && !o.sold)
    }

    /// Active combat particle systems on this frame (host registry snapshot).
    pub fn active_particle_systems(&self) -> impl Iterator<Item = &PresentationParticleSystem> {
        self.particle_systems.iter().filter(|p| p.active)
    }

    /// True when at least one combat particle system is registered and active.
    pub fn has_active_particles(&self) -> bool {
        self.particle_systems.iter().any(|p| p.active)
    }

    /// Active presentation laser beams (assist BinaryDataStream residual).
    pub fn laser_beams(&self) -> &[PresentationLaserBeam] {
        &self.laser_beams
    }

    /// Total Line3D segments across all frozen laser beams.
    pub fn laser_segment_count(&self) -> usize {
        self.laser_beams.iter().map(|b| b.segments.len()).sum()
    }

    /// True when at least one residual laser beam is frozen on this frame.
    pub fn has_active_lasers(&self) -> bool {
        !self.laser_beams.is_empty()
    }

    /// Frozen InGameUI floating texts (host residual observe path).
    pub fn floating_texts(&self) -> &[PresentationFloatingText] {
        &self.floating_texts
    }

    /// Floating texts still within residual timeout at `frame` (or this frame).
    pub fn active_floating_texts_at(&self, logic_frame: u32) -> Vec<&PresentationFloatingText> {
        self.floating_texts
            .iter()
            .filter(|t| t.is_active_at(logic_frame))
            .collect()
    }

    /// True when at least one floating text is frozen on this frame.
    pub fn has_floating_texts(&self) -> bool {
        !self.floating_texts.is_empty()
    }

    /// Host-testable floating text residual usable for dual-tick UI layout pack.
    ///
    /// Empty is honest (no cash events yet). Non-empty requires GUI:AddCash key residual
    /// and positive timeout window.
    pub fn floating_text_presentation_ok(&self) -> bool {
        if self.floating_texts.is_empty() {
            return true;
        }
        self.floating_texts.iter().all(|t| {
            !t.text.is_empty()
                && t.text_key == "GUI:AddCash"
                && t.timeout_frame > t.spawn_frame
                && t.amount > 0
        })
    }

    /// Frozen MoneyPickUp / world Anim2D residuals.
    pub fn world_anims(&self) -> &[PresentationWorldAnim] {
        &self.world_anims
    }

    /// True when at least one world anim is frozen on this frame.
    pub fn has_world_anims(&self) -> bool {
        !self.world_anims.is_empty()
    }

    /// Host-testable world-anim residual usable for dual-tick Anim2D pack.
    ///
    /// Empty is honest. Non-empty requires MoneyPickUp template + positive display.
    pub fn world_anim_presentation_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return true;
        }
        self.world_anims.iter().all(|a| {
            a.template == crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE
                && a.display_time_seconds > 0.0
                && a.z_rise_per_second > 0.0
        })
    }

    /// Host-testable FOW grid residual usable for minimap / terrain texture path.
    ///
    /// Active grids must have a consistent cell buffer; inactive grids are honest
    /// when shroud was not initialized (boot / no-map host).
    pub fn minimap_fow_presentation_ok(&self) -> bool {
        let g = &self.fow_grid;
        if !g.active {
            return true;
        }
        g.cell_count() == (g.width as usize).saturating_mul(g.height as usize)
            && !g.to_r8_texture().is_empty()
    }

    /// Dual-tick residual counters on this frame.
    #[inline]
    pub fn dual_tick(&self) -> &PresentationDualTickResidual {
        &self.dual_tick
    }

    /// Honesty: dual-tick build residual counters are self-consistent.
    pub fn dual_tick_presentation_residual_ok(&self) -> bool {
        self.dual_tick.honesty_build_ok()
            && self.dual_tick.object_count == self.objects.len() as u32
            && self.dual_tick.laser_beam_count == self.laser_beams.len() as u32
            && self.dual_tick.floating_text_count == self.floating_texts.len() as u32
            && self.dual_tick.world_anim_count == self.world_anims.len() as u32
            // Wave 102: selected + particle dual-tick residual counters.
            && self.dual_tick.selected_count == self.selected.len() as u32
            && self.dual_tick.particle_count == self.particle_systems.len() as u32
    }

    /// Wave 102: dual-tick residual deepen honesty (build + apply + content counts).
    ///
    /// Deepens dual-tick bookkeeping beyond Wave 65/75 counters: selected/particle
    /// counts, apply order residual (applies ≥ builds after shell apply), and
    /// cross-link presentation residual packs. Fail-closed vs live dual-run GPU.
    pub fn dual_tick_presentation_residual_deepen_ok(&self) -> bool {
        self.dual_tick_presentation_residual_ok()
            && self.dual_tick.builds >= 1
            && self.floating_text_vanish_residual_ok()
            && self.world_anim_fade_residual_ok()
            && self.laser_presentation_residual_ok()
            && self.spectre_orbit_decal_presentation_residual_ok()
            && self.mesh_scale_presentation_residual_ok()
            && self.ground_height_presentation_residual_ok()
    }

    /// Honesty: floating-text vanish-rate residual fields (empty is honest).
    pub fn floating_text_vanish_residual_ok(&self) -> bool {
        PresentationFloatingText::honesty_vanish_rate_residual_ok()
            && self.floating_texts.iter().all(|t| {
                let a = t.vanish_alpha_at(self.frame.0);
                a.is_finite() && (0.0..=1.0).contains(&a)
            })
    }

    /// Honesty: world-anim fade residual fields (empty is honest).
    pub fn world_anim_fade_residual_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return PresentationWorldAnim::honesty_money_pickup_fade_params_ok();
        }
        self.world_anims
            .iter()
            .all(|a| a.honesty_fade_residual_ok())
    }

    /// Honesty: laser ground-height + multi-beam soft-edge presentation residual.
    pub fn laser_presentation_residual_ok(&self) -> bool {
        self.laser_beams
            .iter()
            .all(|b| b.honesty_ground_height_ok() && b.honesty_soft_edge_presentation_ok())
            && PRESENTATION_ORBITAL_SOFT_EDGE.honesty_orbital_residual_ok()
            && honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
    }

    /// Honesty: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual (Wave 73).
    ///
    /// Constant pack — presentation freezes retail decal defaults so dual-tick
    /// consumers can draw orbit cursors without re-reading live SpectreGunshipUpdate.
    /// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb submit.
    pub fn spectre_orbit_decal_presentation_residual_ok(&self) -> bool {
        let _ = self;
        honesty_spectre_orbit_decal_presentation_ok()
    }

    /// Honesty: mesh scale residual frozen on objects / unit render inputs (Wave 75).
    ///
    /// Common combat units retail-default to **1.0**. Empty snapshot is honest.
    /// Fail-closed: not full Object INI Scale field / draw-scale bone matrix.
    pub fn mesh_scale_presentation_residual_ok(&self) -> bool {
        crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok()
            && self
                .objects
                .iter()
                .all(|o| o.mesh_scale.is_finite() && o.mesh_scale > 0.0)
            && self
                .unit_render_inputs()
                .iter()
                .all(|u| u.mesh_scale.is_finite() && u.mesh_scale > 0.0)
    }

    /// Honesty: unit/structure ground-height residual frozen on objects (Wave 77).
    ///
    /// Empty object lists are honest (default path). Fail-closed: not full
    /// HeightMap bilinear / bridge-aware / locomotor Y rewrite.
    pub fn ground_height_presentation_residual_ok(&self) -> bool {
        honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
            && self.objects.iter().all(|o| {
                honesty_ground_height_residual_ok(o.ground_height, o.ground_height_from_terrain)
            })
    }

    /// Note a dual-tick apply on this snapshot (HUD / shell multi-consumer path).
    pub fn note_dual_tick_apply(&mut self) {
        self.dual_tick.applies = self.dual_tick.applies.saturating_add(1);
    }
}
