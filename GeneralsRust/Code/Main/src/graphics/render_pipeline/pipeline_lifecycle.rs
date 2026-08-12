#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

impl RenderPipeline {
    /// Initialize render pipeline - equivalent to C++ RenderPipeline::Initialize()
    pub fn initialize(graphics_system: &GraphicsSystem) -> Result<Self> {
        info!("Initializing RenderPipeline (C++ SAGE equivalent)");

        // Initialize forward pass
        let forward_pass = ForwardPass::initialize(graphics_system)?;
        let (ambient_light, sun_color, sun_direction) = graphics_system.current_lighting();

        info!("RenderPipeline initialized successfully");

        Ok(Self {
            forward_pass,
            minimap_renderer: None, // Will be initialized when needed
            minimap_base_needs_refresh: false,
            heightmap_path_hint: None,
            pending_heightmap_hint_load: false,
            skybox_textures_hint: None,
            skybox_enabled: true,
            heightmap_world_size: None,
            cached_lighting: Some(CachedLighting {
                sun_direction: Some(sun_direction),
                sun_color: Some(sun_color),
                ambient_color: Some(ambient_light),
                fog_color: None,
                fog_range: None,
            }),
            last_startup_model_prewarm_signature: None,
            render_items: Vec::new(),
            frame_number: 0,
            current_pass: None,
            current_player_id: 0, // Default to player 0
            missing_ini_objects: HashSet::new(),
            debug_last_alive_objects: 0,
            debug_last_live_unit_identity_reads: 0,
            debug_last_presentation_live_fallback_reads: 0,
            debug_last_fow_filtered: 0,
            debug_last_frustum_culled: 0,
            debug_last_model_missing: 0,
            debug_last_deferred_model_loads: 0,
            debug_last_deferred_model_load_budget: 0,
            debug_last_model_budget_skips: 0,
            debug_last_zero_mesh_models: 0,
            debug_last_missing_model_samples: Vec::new(),
            debug_warned_bad_mesh_transforms: HashSet::new(),
            model_cull_bounds_cache: HashMap::new(),
            animation_states: HashMap::new(),
            last_frame_time: 0.0,
            presentation_frame: None,
            debug_last_laser_segments_packed: 0,
            debug_last_laser_pack_ok: false,
            debug_last_laser_gpu_write_ok: false,
            debug_last_projectile_segments_packed: 0,
            debug_last_projectile_pack_ok: false,
            debug_last_move_lines_packed: 0,
            debug_last_attack_lines_packed: 0,
            debug_last_floating_texts_packed: 0,
            debug_last_floating_text_pack_ok: false,
            debug_last_world_anims_packed: 0,
            debug_last_world_anim_pack_ok: false,
            debug_last_particle_systems_packed: 0,
            debug_last_particle_pack_ok: false,
        })
    }

    /// Provide full presentation snapshot for the next collect_render_items pass.
    pub fn set_presentation_frame(
        &mut self,
        frame: Option<crate::presentation_frame::PresentationFrame>,
    ) {
        self.presentation_frame = frame;
    }

    #[inline]
    pub fn presentation_frame(&self) -> Option<&crate::presentation_frame::PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    #[inline]
    pub fn presentation_frame_mut(
        &mut self,
    ) -> Option<&mut crate::presentation_frame::PresentationFrame> {
        self.presentation_frame.as_mut()
    }

    /// Live GameLogic identity reads during last unit mesh collect (0 when presentation owns pass).
    pub fn last_live_unit_identity_reads(&self) -> usize {
        self.debug_last_live_unit_identity_reads
    }

    /// Live GameLogic dual-reads observed while presentation_frame was installed.
    /// Honesty residual: must remain 0 on the presentation-owned path.
    pub fn last_presentation_live_fallback_reads(&self) -> usize {
        self.debug_last_presentation_live_fallback_reads
    }

    /// Residual: presentation dual-read honesty — must stay 0 when frame installed.
    pub fn presentation_live_fallback_honesty_ok(&self) -> bool {
        self.debug_last_presentation_live_fallback_reads == 0
    }

    /// Pure unit-identity + FOW collection for the main mesh pass (no GameLogic borrow).
    ///
    /// Production `collect_render_items` uses this when a presentation frame is set.
    /// W3D mesh asset load remains outside this helper.
    pub fn collect_unit_render_inputs_from_presentation(
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> Vec<crate::presentation_frame::UnitRenderInput> {
        // Wave 502: stealth filter/alpha applied inside unit_render_inputs (presentation-only).
        // Wave 503: disguise mesh swap + construction bits via stamp helper.
        // Wave 504: contained units filtered; garrisoned bits in stamp helper.
        // Wave 505: parachuting/jetexhaust/using-weapon stamp via unit_render_inputs.
        // Wave 506: weaponset veterancy stamp via unit_render_inputs.
        // Wave 507: over-water + transport RIDER stamp via unit_render_inputs.
        // Wave 508: body-damage / disguise / stun stamp via unit_render_inputs.
        // Wave 509: topple/freefall/night/snow stamp via unit_render_inputs.
        // Wave 510: captured/loaded/overcharge stamp via unit_render_inputs.
        // Wave 511: burned/aflame/cheer/carry stamp via unit_render_inputs.
        // Wave 512: continuous-fire/prone/preattack/turret stamp via unit_render_inputs.
        // Wave 513: jammed/dying/reload/packing stamp via unit_render_inputs.
        // Wave 515: surrender/raising-flag stamp via unit_render_inputs.
        frame.unit_render_inputs()
    }

    /// Backward-compatible: store IDs-only by building a minimal frame is not needed;
    /// prefer set_presentation_frame. Kept as thin alias for call sites.
    pub fn set_presentation_object_ids(&mut self, ids: Option<Vec<ObjectID>>) {
        if ids.is_none() {
            self.presentation_frame = None;
        }
        // IDs-only path no longer used; clear frame when None.
    }
}
