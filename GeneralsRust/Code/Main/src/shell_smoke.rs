//! Production host smoke: SkirmishMenu → config → apply → map load → frames → presentation.
//!
//! Full windowed shell/WND + GPU boot still requires a display; this path exercises the
//! same production APIs `start_game_from_ui` uses after menu StartGame.
//!
//! Honesty: no tautological host flag, no silent golden_skirmish_config fallback.
//! Opponent slot is configured through SkirmishMenu::configure_slot_medium_ai.
//!
//! Claim flags (do not conflate):
//! - `playable_claim` — **always false**. Headless host APIs are not retail W3D /
//!   windowed shell playthrough. Fail-closed pending full GPU/WND match play.
//! - `shell_host_playable_ok` — limited honesty claim: when true, the headless
//!   shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd
//!   ensure path is operational. Still **not** a retail playthrough claim.
//!
//! Residual honesty (do **not** flip `playable_claim`):
//! - `dual_tick_presentation_ok` — seed + logic update + multi-consumer presentation apply
//! - `dual_tick_counters_ok` — presentation dual-tick residual counters (build/apply)
//! - `minimap_fow_presentation_ok` — FOW grid snapshot usable for minimap texture path
//! - `laser_segment_upload_ok` — presentation → CPU SegLine pack residual (incl. synthetic)
//! - `multi_beam_soft_edge_ok` — OrbitalLaser NumBeams soft-edge CPU pack residual
//! - `laser_presentation_residual_ok` — ground-height + soft-edge presentation fields
//! - `floating_text_layout_ok` — presentation → CPU InGameUI floating-text layout residual
//! - `floating_text_vanish_ok` — vanish-rate alpha residual presentation field honesty
//! - `world_anim_presentation_ok` — MoneyPickUp Anim2D residual frozen on presentation
//! - `world_anim_layout_ok` — presentation → CPU Anim2D layout pack residual
//! - `world_anim_fade_ok` — world-anim fade residual presentation field honesty
//! - `anim2d_frame_ok` — MoneyPickUp Anim2D frame advance residual
//! - `anim2d_collection_residual_ok` — Anim2DCollection template/instance residual
//! - `translate_copy_residual_ok` — GameText translate_copy escape table residual
//! - `game_text_caption_ok` — GUI:AddCash caption residual on floating-text pack
//! - `game_text_csf_str_ok` — CSF/STR parse + retail `$%d` printf + DisplayString measure
//! - `display_string_measure_ok` — monospaced glyph measure residual on floating-text pack
//! - `rng_stream_residual_ok` — GameLogic/GameClient RandomValue ADC stream residual
//! - `mesh_asset_residual_ok` — W3D mesh resolve residual (keys/scale/search; no GPU)
//! - `rng_residual_pack_ok` — Wave 72 host RNG residual pack honesty
//! - `special_power_wave72_residual_ok` — Daisy/A10 special-power residual pack
//! - `special_power_wave73_residual_ok` — Spectre/Nuke/SupW residual pack
//! - `special_power_wave76_residual_ok` — A10 science-tier FormationSize residual pack
//! - `paradrop_wave76_residual_ok` — Paradrop science-tier payload residual pack
//! - `control_bar_wave76_residual_ok` — ControlBar window-count/named/font residual pack
//! - `graphics_wave76_residual_ok` — InGameUI font table + vanish color-alpha residual
//! - `spectre_orbit_decal_presentation_ok` — Wave 73 presentation Spectre decal residual
//! - `special_power_wave77_residual_ok` — Wave 77 audio name tables residual pack
//! - `special_power_wave78_residual_ok` — Wave 78 reload table / CarpetBomb / Artillery residual pack
//! - `cluster_mines_wave78_residual_ok` — Wave 78 ClusterMines DeliveryDecal / science residual pack
//! - `gps_scrambler_wave78_residual_ok` — Wave 78 GPS science / marker particle residual pack
//! - `cash_bounty_wave78_residual_ok` — Wave 78 CashBountyScienceTier residual pack
//! - `fow_residual_pack_ok` — Wave 77 FOW cell/R8/inactive residual honesty
//! - `ground_height_presentation_ok` — Wave 77 unit ground-height presentation residual
//! - `weapon_store_seed_residual_ok` — Wave 77 host WeaponStore seed residual pack
//! - `ai_skirmish_residual_ok` — Wave 77 AI skirmish timer/wealth residual pack
//! - `minimap_residual_pack_ok` — Wave 79 minimap FOW shade/size residual pack
//! - `selection_hud_residual_pack_ok` — Wave 79 selection/HUD color residual pack
//! - `input_residual_pack_ok` — Wave 79 drag/double-click input residual pack
//! - `drawable_residual_fields_ok` — Wave 79 Drawable StealthLook save/load residual
//! - `unit_training_wave79_residual_ok` — Wave 79 veterancy bonus / AdvancedTraining XP
//! - `upgrades_cost_time_application_ok` — Wave 79 upgrade cost/time application residual
//! - `control_bar_path_resolved` / `control_bar_wnd_validated` — ControlBar.wnd residual
//! - `control_bar_window_loaded` — headless WindowManager parse when WindowZH present

use crate::ai_skirmish_activity::honesty_ai_skirmish_residual_pack_wave77;
use crate::assets::mesh_asset_resolve::honesty_mesh_asset_residual_ok;
use crate::fow_rendering::honesty_fow_residual_pack_wave77;
use crate::game_logic::host_paradrop::honesty_paradrop_residual_pack_wave76_ok;
use crate::game_logic::host_rng_residual::{
    exercise_host_rng_residual, honesty_rng_residual_pack_ok,
};
use crate::game_logic::host_cash_bounty::honesty_cash_bounty_residual_pack_wave78;
use crate::game_logic::host_gps_scrambler::honesty_gps_scrambler_residual_pack_wave78;
use crate::game_logic::host_mines::honesty_cluster_mines_residual_pack_wave78;
use crate::game_logic::host_unit_training::honesty_unit_training_residual_pack_wave79_ok;
use crate::game_logic::host_upgrades::honesty_upgrades_cost_time_application_wave79_ok;
use crate::game_logic::special_power_strikes::{
    honesty_special_power_residual_pack_ok, honesty_special_power_residual_pack_wave73_ok,
    honesty_special_power_residual_pack_wave76_ok, honesty_special_power_residual_pack_wave77_ok,
    honesty_special_power_residual_pack_wave78_ok,
};
use crate::game_logic::weapon_bootstrap::honesty_weapon_store_host_seed_residual_wave77;
use crate::game_logic::GameLogic;
use crate::graphics::minimap_renderer::honesty_minimap_residual_pack_wave79;
use crate::presentation_frame::honesty_spectre_orbit_decal_presentation_ok;
use crate::save_load::honesty_drawable_residual_fields_wave79_ok;
use crate::selection_renderer::honesty_selection_hud_residual_pack_wave79;
use crate::unit_input_handler::honesty_input_residual_pack_wave79;
use crate::gameplay_layout::{
    control_bar_layout_honesty, format_control_bar_honesty,
    honesty_control_bar_residual_pack_wave76_ok, GameplayLayoutStatus,
};
use crate::graphics::floating_text_layout::{
    honesty_graphics_residual_pack_wave76_ok, pack_floating_text_and_mark_ready, FloatingTextLayout,
};
use crate::graphics::game_text_residual::{
    exercise_host_game_text_residual, honesty_translate_copy_escape_table,
};
use crate::graphics::laser_segment_upload::{
    pack_and_mark_upload_ready, LaserSegmentUpload,
};
use crate::graphics::world_anim_layout::{
    honesty_anim2d_collection_residual, pack_world_anim_and_mark_ready, WorldAnimLayout,
};
use crate::map_frame_scenario::resolve_first_map;
use crate::presentation_frame::{
    PresentationFloatingText, PresentationFrame, PresentationLaserBeam, PresentationWorldAnim,
    PRESENTATION_ORBITAL_SOFT_EDGE,
};
use crate::skirmish_config::{apply_skirmish_config, config_from_skirmish_menu};
use crate::ui::skirmish_menu::SkirmishMenu;
use crate::ui::{
    GameHUD, GameUIState, RTSInterface, Screen, UIManager, UnitCommandPanel,
};

const HOST_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    /// True only after GameLogic exists and skirmish config applied successfully.
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub presentation_ok: bool,
    /// Dual-tick residual: seed presentation, then logic update + build_and_apply order.
    pub dual_tick_presentation_ok: bool,
    /// Dual-tick residual counters (build/apply) on presentation snapshot.
    pub dual_tick_counters_ok: bool,
    /// Dual-tick residual: after map load, logic update + presentation seed HUD
    /// selection health / minimap without re-reading live objects.
    pub hud_selection_ok: bool,
    /// Minimap FOW residual: presentation `fow_grid` is host-usable (active R8 or honest inactive).
    pub minimap_fow_presentation_ok: bool,
    /// Laser residual: presentation → CPU SegLine vertex pack (+ synthetic non-empty pack).
    pub laser_segment_upload_ok: bool,
    /// OrbitalLaser multi-beam soft-edge CPU pack residual (NumBeams width/color lerp).
    pub multi_beam_soft_edge_ok: bool,
    /// Laser presentation residual: ground-height + soft-edge fields honesty.
    pub laser_presentation_residual_ok: bool,
    /// Floating-text residual: presentation freeze + CPU layout pack (+ synthetic).
    pub floating_text_layout_ok: bool,
    /// Floating-text vanish-rate alpha residual presentation field honesty.
    pub floating_text_vanish_ok: bool,
    /// MoneyPickUp Anim2D residual: presentation freeze honesty (empty or template ok).
    pub world_anim_presentation_ok: bool,
    /// World-anim residual: presentation → CPU Anim2D layout pack (+ synthetic).
    pub world_anim_layout_ok: bool,
    /// World-anim fade residual presentation field honesty.
    pub world_anim_fade_ok: bool,
    /// MoneyPickUp Anim2D frame advance residual (LOOP / SCPDollarNNN).
    pub anim2d_frame_ok: bool,
    /// Anim2DCollection template/instance residual (host-testable, no GPU).
    pub anim2d_collection_residual_ok: bool,
    /// GameText translate_copy escape table residual (host-testable, no GPU).
    pub translate_copy_residual_ok: bool,
    /// GameText `GUI:AddCash` caption residual on synthetic floating-text pack.
    pub game_text_caption_ok: bool,
    /// CSF/STR GameText residual + retail `$%d` printf + DisplayString measure.
    pub game_text_csf_str_ok: bool,
    /// DisplayString monospaced measure residual on floating-text pack.
    pub display_string_measure_ok: bool,
    /// GameLogic/GameClient RandomValue ADC stream residual honesty.
    pub rng_stream_residual_ok: bool,
    /// W3D mesh asset resolve residual (common keys / scale / search / basename).
    /// Host-testable; does **not** claim live GPU upload or retail material parity.
    pub mesh_asset_residual_ok: bool,
    /// Wave 72 host RNG residual pack honesty (seed table / pure index / stream).
    pub rng_residual_pack_ok: bool,
    /// Wave 72 special-power residual pack (DaisyCutter / A10 / free pack).
    pub special_power_wave72_residual_ok: bool,
    /// Wave 73 Spectre/Nuke/SupW special-power residual pack honesty.
    pub special_power_wave73_residual_ok: bool,
    /// Wave 76 A10 science-tier FormationSize residual pack honesty.
    pub special_power_wave76_residual_ok: bool,
    /// Wave 76 Paradrop science-tier payload residual pack honesty.
    pub paradrop_wave76_residual_ok: bool,
    /// Wave 76 ControlBar window-count / named-child / font residual pack honesty.
    pub control_bar_wave76_residual_ok: bool,
    /// Wave 76 InGameUI font table + DisplayString vanish color-alpha residual honesty.
    pub graphics_wave76_residual_ok: bool,
    /// Wave 73 presentation Spectre orbit decal residual honesty.
    pub spectre_orbit_decal_presentation_ok: bool,
    /// Wave 77 special-power audio name table residual honesty.
    pub special_power_wave77_residual_ok: bool,
    /// Wave 77 FOW residual honesty pack (cell/R8/inactive fail-open).
    pub fow_residual_pack_ok: bool,
    /// Wave 77 unit/structure ground-height presentation residual honesty.
    pub ground_height_presentation_ok: bool,
    /// Wave 77 host WeaponStore seed residual honesty pack.
    pub weapon_store_seed_residual_ok: bool,
    /// Wave 77 AI skirmish structure/team timer residual honesty pack.
    pub ai_skirmish_residual_ok: bool,
    /// Wave 78 HostSuperweaponKind reload + CarpetBomb/Artillery science residual pack.
    pub special_power_wave78_residual_ok: bool,
    /// Wave 78 ClusterMines DeliveryDecal / science residual pack.
    pub cluster_mines_wave78_residual_ok: bool,
    /// Wave 78 GPS Scrambler science / marker particle residual pack.
    pub gps_scrambler_wave78_residual_ok: bool,
    /// Wave 78 CashBountyScienceTier residual pack.
    pub cash_bounty_wave78_residual_ok: bool,
    /// Wave 79 minimap FOW shade/size residual honesty pack.
    pub minimap_residual_pack_ok: bool,
    /// Wave 79 selection/HUD color residual honesty pack.
    pub selection_hud_residual_pack_ok: bool,
    /// Wave 79 drag/double-click input residual honesty pack.
    pub input_residual_pack_ok: bool,
    /// Wave 79 Drawable StealthLook save/load residual honesty.
    pub drawable_residual_fields_ok: bool,
    /// Wave 79 unit-training/veterancy residual deepen honesty pack.
    pub unit_training_wave79_residual_ok: bool,
    /// Wave 79 upgrade cost/time residual application honesty.
    pub upgrades_cost_time_application_ok: bool,
    /// Shell Skirmish → Loading → GameHUD ownership transition (StartGame parity).
    pub screen_skirmish_ok: bool,
    /// ControlBar.wnd resolve/validate path (C++ ShowControlBar / ensure_gameplay_layouts).
    /// True when layout Ready, or assets honestly unavailable (CI without WindowZH).
    pub control_bar_layout_ok: bool,
    /// ControlBar.wnd path found on disk.
    pub control_bar_path_resolved: bool,
    /// ControlBar.wnd structural validate (FILE_VERSION / WINDOW / ControlBar tokens).
    pub control_bar_wnd_validated: bool,
    /// Headless WindowManager parse materialised GameWindows (assets present path).
    /// False when WindowZH missing or parse deferred — still honest residual.
    pub control_bar_window_loaded: bool,
    /// Window count from headless WindowManager load (0 when not loaded).
    pub control_bar_window_count: usize,
    /// Dual-tick residual: selection panel applied to HUD + UIState + RTS + command panel.
    pub selection_consumers_ok: bool,
    /// Limited headless host claim (see module docs). Not retail W3D play.
    pub shell_host_playable_ok: bool,
    /// Always false here: no window/WND/GPU retail playthrough.
    pub playable_claim: bool,
    pub status: String,
    pub detail: String,
}

/// Exercise production host entry points headlessly (no window required).
/// Builds config from live SkirmishMenu (including Medium AI slot via menu cycle),
/// applies it, loads retail map when present, advances logic frames, builds presentation,
/// applies dual-tick presentation → GameHUD selection/minimap, ensures ControlBar.wnd,
/// and exercises shell→InGame screen ownership (start_game_from_ui parity).
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    let mut logic = GameLogic::new();

    let resolved = resolve_first_map(HOST_MAP_CANDIDATES);
    let map_resolved = resolved.is_some();
    let map_id = resolved
        .as_ref()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "HostSyntheticMap".into());
    let map_path = resolved.map(|(_, p)| p);

    // Production UI path only — no golden_skirmish_config fallback.
    let mut menu = SkirmishMenu::new();
    let menu_init_ok = menu.initialize().is_ok();
    // Slot 0 is Human by default; configure slot 1 as Medium AI via menu cycling.
    let medium_ai_ok = menu.configure_slot_medium_ai(1);
    if map_resolved {
        menu.set_map_name(map_id.clone());
    }
    let (slots, rules, menu_map_name) = menu.get_game_config();
    let cfg = config_from_skirmish_menu(&menu_map_name, &rules, &slots);
    let active = cfg.slots.iter().filter(|s| s.is_active).count();
    let has_human = cfg.slots.iter().any(|s| s.is_human);
    let has_ai = cfg.slots.iter().any(|s| !s.is_human && s.is_active);
    let menu_config_ok = menu_init_ok && medium_ai_ok && active >= 2 && has_human && has_ai;

    let apply_ok = apply_skirmish_config(&mut logic, &cfg).is_ok();
    let skirmish_config_ok = apply_ok
        && logic.get_players().len() >= 2
        && logic.host_ai_player_count() >= 1
        && logic.skirmish_rules().fog_of_war;

    // Host is "constructed" only when production apply path succeeds — not a constant true.
    let host_constructed = skirmish_config_ok;

    let map_loaded = if let Some(ref path) = map_path {
        logic.load_map(&path.display().to_string())
    } else {
        false
    };

    // Immediate post-map seed (matches start_game_from_ui seed before first dual-tick).
    // Multi-consumer residual: HUD + UIState + RTS + unit command panel share snapshot.
    let mut hud = GameHUD::new();
    let mut ui_state = GameUIState::default();
    let mut rts = RTSInterface::new();
    let mut command_panel = UnitCommandPanel::new();
    let seed_pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let seed_ok = seed_pres.frame.0 == logic.get_frame()
        && (seed_pres.alive_object_count() > 0 || !map_loaded);

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        // Dual-tick: authority step then multi-consumer presentation apply.
        logic.update();
        let _ = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic,
            0,
            &mut hud,
            &mut ui_state,
            &mut rts,
            &mut command_panel,
        );
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced > 0;

    // Ensure at least one selectable unit is selected so selection health is exercised.
    let select_id = logic
        .get_objects()
        .values()
        .find(|o| o.is_alive() && !o.status.destroyed)
        .map(|o| o.id);
    if let Some(id) = select_id {
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
    }

    let pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let presentation_ok = seed_ok
        && pres.frame.0 == logic.get_frame()
        && (pres.alive_object_count() > 0 || !map_loaded)
        && !pres
            .objects
            .iter()
            .any(|o| o.model_key.is_none() && !o.destroyed);

    // Dual-tick residual honesty: seed frame applied, then post-update presentation
    // matches authority frame (start_game_from_ui / engine dual-tick order).
    let dual_tick_presentation_ok = seed_ok
        && frames_ok
        && presentation_ok
        && pres.frame.0 == logic.get_frame()
        && seed_pres.frame.0 <= pres.frame.0;
    // Dual-tick residual counters (build + apply recorded on shell apply path).
    let dual_tick_counters_ok = presentation_ok
        && dual_tick_presentation_ok
        && seed_pres.dual_tick_presentation_residual_ok()
        && seed_pres.dual_tick.honesty_apply_ok()
        && pres.dual_tick_presentation_residual_ok()
        && pres.dual_tick.honesty_apply_ok()
        && seed_pres.dual_tick.applies >= 1
        && pres.dual_tick.applies >= 1;

    // Minimap FOW from presentation residual (grid snapshot, not live shroud re-lock).
    let minimap_fow_presentation_ok = presentation_ok && pres.minimap_fow_presentation_ok();

    // WGPU laser segment upload residual (CPU pack path; no live device required).
    // Empty host lasers → honest empty pack; synthetic assist pair exercises geometry.
    let empty_pack = pack_and_mark_upload_ready(&pres);
    let synthetic = PresentationLaserBeam::synthetic_assist_pair(pres.frame.0);
    let mut synth_frame = pres.clone();
    synth_frame.laser_beams = synthetic.to_vec();
    let synth_pack = LaserSegmentUpload::pack_from_presentation(&synth_frame);
    let laser_segment_upload_ok = empty_pack.honesty.honesty_cpu_pack_ok()
        && empty_pack.honesty.honesty_upload_ready_ok()
        && synth_pack.honesty.honesty_geometry_ok()
        && synth_pack.honesty.segments_packed >= 20
        && synth_pack.honesty.beams_packed == 2
        && synthetic[0].honesty_ground_height_ok()
        && synthetic[0].honesty_soft_edge_presentation_ok();
    // OrbitalLaser multi-beam soft-edge: presentation residual fields → CPU pack.
    let orbital = PresentationLaserBeam::synthetic_orbital_soft_edge(pres.frame.0);
    let se = orbital.soft_edge.unwrap_or(PRESENTATION_ORBITAL_SOFT_EDGE);
    let (mb_start, mb_end, mb_elapsed, mb_width) =
        se.pack_endpoints(orbital.from, orbital.to, 1.0);
    let multi_beam_pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
        mb_start, mb_end, mb_elapsed, mb_width,
    );
    let multi_beam_soft_edge_ok = multi_beam_pack.honesty.honesty_cpu_pack_ok()
        && multi_beam_pack.honesty.honesty_geometry_ok()
        && multi_beam_pack.honesty.honesty_multi_beam_soft_edge_ok()
        && orbital.honesty_soft_edge_presentation_ok()
        && se.honesty_orbital_residual_ok();
    let laser_presentation_residual_ok =
        presentation_ok && pres.laser_presentation_residual_ok() && multi_beam_soft_edge_ok;

    // InGameUI floating text + MoneyPickUp Anim2D residual (CPU layout; no live GPU).
    // Empty host texts → honest empty pack; synthetic cash exercises geometry.
    let ft_empty = pack_floating_text_and_mark_ready(&pres);
    let mut ft_synth_frame = pres.clone();
    ft_synth_frame.floating_texts = vec![PresentationFloatingText::synthetic_cash(100, pres.frame.0)];
    ft_synth_frame.world_anims = vec![PresentationWorldAnim::synthetic_money_pickup(pres.frame.0)];
    let ft_synth = FloatingTextLayout::pack_from_presentation(&ft_synth_frame);
    let floating_text_layout_ok = presentation_ok
        && pres.floating_text_presentation_ok()
        && ft_empty.honesty.honesty_cpu_pack_ok()
        && ft_empty.honesty.honesty_upload_ready_ok()
        && ft_empty.honesty.honesty_retail_params_ok()
        && ft_synth.honesty.honesty_geometry_ok()
        && ft_synth.honesty.texts_packed == 1
        && ft_synth.honesty.world_anims_observed == 1;
    let floating_text_vanish_ok = floating_text_layout_ok
        && pres.floating_text_vanish_residual_ok()
        && PresentationFloatingText::honesty_vanish_rate_residual_ok()
        && PresentationFloatingText::honesty_vanish_color_alpha_residual_ok()
        && ft_synth_frame.floating_texts.iter().all(|t| {
            let a = t.vanish_alpha_at(pres.frame.0);
            (a - 1.0).abs() < 0.001
        });
    let game_text_caption_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_game_text_caption_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.caption == "+$100")
            .unwrap_or(false);
    let display_string_measure_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_display_string_measure_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.measure_width > 0 && e.measure_height == 8)
            .unwrap_or(false);
    // CSF/STR GameText residual exercise (retail `$%d` + optional live CSF).
    let game_text_csf_str_ok = exercise_host_game_text_residual().honesty.honesty_ok();
    // translate_copy escape table residual (host-testable, no GPU).
    let translate_copy_residual_ok = honesty_translate_copy_escape_table();
    let world_anim_presentation_ok = presentation_ok && pres.world_anim_presentation_ok();
    // World-anim CPU layout residual (empty + synthetic MoneyPickUp).
    let wa_empty = pack_world_anim_and_mark_ready(&pres);
    let wa_synth = WorldAnimLayout::pack_from_presentation(&ft_synth_frame);
    let world_anim_layout_ok = presentation_ok
        && world_anim_presentation_ok
        && wa_empty.honesty.honesty_cpu_pack_ok()
        && wa_empty.honesty.honesty_upload_ready_ok()
        && wa_synth.honesty.honesty_geometry_ok()
        && wa_synth.honesty.anims_packed == 1
        && wa_synth.honesty.honesty_template_ok();
    let world_anim_fade_ok = world_anim_layout_ok
        && pres.world_anim_fade_residual_ok()
        && PresentationWorldAnim::honesty_money_pickup_fade_params_ok()
        && ft_synth_frame
            .world_anims
            .iter()
            .all(|a| a.honesty_fade_residual_ok());
    let anim2d_frame_ok = world_anim_layout_ok
        && wa_synth.honesty.honesty_anim2d_frame_ok()
        && wa_synth
            .entries
            .first()
            .map(|e| e.frame_image.starts_with("SCPDollar"))
            .unwrap_or(false);
    // Anim2DCollection residual (host-testable, no GPU).
    let anim2d_collection_residual_ok = honesty_anim2d_collection_residual();
    // GameLogic / GameClient RandomValue ADC stream residual.
    let rng_stream_residual_ok = exercise_host_rng_residual(0x5A6E_2710).honesty_ok();
    // Wave 75 mesh / wave 72–73 residual honesty (host-testable, no GPU claim).
    let mesh_asset_residual_ok = honesty_mesh_asset_residual_ok();
    let rng_residual_pack_ok = honesty_rng_residual_pack_ok();
    let special_power_wave72_residual_ok = honesty_special_power_residual_pack_ok();
    let special_power_wave73_residual_ok = honesty_special_power_residual_pack_wave73_ok();
    let special_power_wave76_residual_ok = honesty_special_power_residual_pack_wave76_ok();
    let paradrop_wave76_residual_ok = honesty_paradrop_residual_pack_wave76_ok();
    let graphics_wave76_residual_ok = honesty_graphics_residual_pack_wave76_ok();
    let spectre_orbit_decal_presentation_ok = honesty_spectre_orbit_decal_presentation_ok()
        && presentation_ok
        && pres.spectre_orbit_decal_presentation_residual_ok();
    // Wave 77 residual honesty packs (orthogonal to ControlBar/script; no playable_claim flip).
    let special_power_wave77_residual_ok = honesty_special_power_residual_pack_wave77_ok();
    let fow_residual_pack_ok = honesty_fow_residual_pack_wave77();
    let ground_height_presentation_ok =
        presentation_ok && pres.ground_height_presentation_residual_ok();
    let weapon_store_seed_residual_ok = honesty_weapon_store_host_seed_residual_wave77();
    let ai_skirmish_residual_ok = honesty_ai_skirmish_residual_pack_wave77();
    // Wave 78 residual honesty packs (reload table + science tiers; no playable_claim flip).
    let special_power_wave78_residual_ok = honesty_special_power_residual_pack_wave78_ok();
    let cluster_mines_wave78_residual_ok = honesty_cluster_mines_residual_pack_wave78();
    let gps_scrambler_wave78_residual_ok = honesty_gps_scrambler_residual_pack_wave78();
    let cash_bounty_wave78_residual_ok = honesty_cash_bounty_residual_pack_wave78();
    // Wave 79 residual honesty packs (orthogonal to special powers; no playable_claim flip).
    let minimap_residual_pack_ok = honesty_minimap_residual_pack_wave79();
    let selection_hud_residual_pack_ok = honesty_selection_hud_residual_pack_wave79();
    let input_residual_pack_ok = honesty_input_residual_pack_wave79();
    let drawable_residual_fields_ok = honesty_drawable_residual_fields_wave79_ok();
    let unit_training_wave79_residual_ok = honesty_unit_training_residual_pack_wave79_ok();
    let upgrades_cost_time_application_ok = honesty_upgrades_cost_time_application_wave79_ok();

    // HUD + multi-consumer selection panel health from presentation after dual-tick.
    let (hud_selection_ok, selection_consumers_ok) = if let Some(id) = select_id {
        let infos = hud.selected_unit_infos();
        let snap_infos = pres.selected_unit_display_infos();
        let hud_hit = infos.iter().any(|u| {
            u.object_id == id && u.health_current > 0.0 && u.health_maximum >= u.health_current
        });
        let snap_hit = snap_infos
            .iter()
            .any(|u| u.object_id == id && u.health_current > 0.0);
        let ids_ok = hud.selected_unit_ids().contains(&id);
        let minimap_ok = !pres.hud_minimap_units().is_empty() || !map_loaded;
        let panel = hud.selection_panel();
        let panel_ok = panel.visible
            && panel.has_positive_health()
            && panel.primary_object_id == Some(id);
        // Optional ControlBar path (headless selection health; not full WND claim).
        #[cfg(feature = "game_client")]
        let control_bar_ok = {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            pres.apply_to_control_bar(&mut bar);
            bar.selection_panel_health()
                .map(|(hp, max)| hp > 0.0 && max >= hp)
                .unwrap_or(false)
        };
        #[cfg(not(feature = "game_client"))]
        let control_bar_ok = true;
        let ui_ok = ui_state.selection_panel.has_positive_health()
            && ui_state.selection_panel.primary_object_id == Some(id);
        let rts_ok = rts.selection_panel().has_positive_health() && rts.selected_ids().contains(&id);
        let cmd_ok = command_panel.is_visible()
            && command_panel.selection_panel().has_positive_health()
            && command_panel.selected_ids().contains(&id);
        let consumers_ok = ui_ok && rts_ok && cmd_ok && control_bar_ok;
        (
            hud_hit && snap_hit && ids_ok && minimap_ok && panel_ok && control_bar_ok,
            consumers_ok,
        )
    } else {
        // No objects (absent-map synthetic host): still require resource apply path.
        let empty_ok = hud.selected_unit_ids().is_empty()
            && !hud.selection_panel().visible
            && (pres.local_supplies > 0 || skirmish_config_ok);
        let consumers_empty = !ui_state.selection_panel.visible
            && rts.selected_ids().is_empty()
            && !command_panel.is_visible();
        (empty_ok, empty_ok && consumers_empty)
    };

    // Shell → InGame residual: production StartGame transitions Skirmish→Loading→GameHUD
    // and ensure_gameplay_layouts (ControlBar.wnd) on InGame enter.
    let mut ui_mgr = UIManager::new(1024, 768);
    ui_mgr.transition_to_screen(Screen::Skirmish);
    let at_skirmish = ui_mgr.current_screen() == Some(Screen::Skirmish)
        && Screen::Skirmish.is_shell_owned_pregame();
    ui_mgr.transition_to_screen(Screen::Loading);
    let at_loading = ui_mgr.current_screen() == Some(Screen::Loading)
        && !Screen::Loading.is_shell_owned_pregame();
    // Attempt headless WindowManager load when game_client is enabled (ShowControlBar
    // residual). AssetsUnavailable remains honest when WindowZH is not checked out.
    // This is **not** windowed W3D retail — only layout script → window tree.
    #[cfg(feature = "game_client")]
    let layout_honesty = control_bar_layout_honesty(true);
    #[cfg(not(feature = "game_client"))]
    let layout_honesty = control_bar_layout_honesty(false);
    let layout_status = layout_honesty.status.clone();
    let layout_report = format_control_bar_honesty(&layout_honesty);
    let control_bar_path_resolved = layout_honesty.path_resolved;
    let control_bar_wnd_validated = layout_honesty.wnd_validated;
    let control_bar_window_loaded = layout_honesty.window_loaded;
    let control_bar_window_count = layout_honesty.window_count;
    let control_bar_layout_ok = match &layout_status {
        GameplayLayoutStatus::Ready { path, loaded } => {
            // Ready after structural validate. Prefer WindowManager load when assets
            // present (`loaded=true`); validated-only (`loaded=false`) is still ok.
            path.contains("ControlBar")
                && control_bar_wnd_validated
                && (*loaded == control_bar_window_loaded)
                && (!*loaded || control_bar_window_count > 0)
        }
        // Honest residual when WindowZH assets are not checked out.
        GameplayLayoutStatus::AssetsUnavailable { searched } => {
            !searched.is_empty()
                && layout_honesty.assets_unavailable
                && !control_bar_window_loaded
        }
        GameplayLayoutStatus::LoadFailed { .. } => false,
    };
    let control_bar_wave76_residual_ok = honesty_control_bar_residual_pack_wave76_ok(
        control_bar_window_loaded,
        control_bar_window_count,
    );
    ui_mgr.transition_to_screen(Screen::GameHUD);
    let at_ingame = ui_mgr.current_screen() == Some(Screen::GameHUD)
        && !Screen::GameHUD.is_shell_owned_pregame();
    let screen_skirmish_ok = at_skirmish
        && at_loading
        && at_ingame
        && Screen::MainMenu.is_shell_owned_pregame()
        && Screen::startup_entry_screen(true) == Screen::MainMenu;

    // When assets present, map must load; when absent, still pass config+frames.
    let map_requirement_ok = if map_resolved { map_loaded } else { true };

    // Never claim full retail playability from headless smoke (no W3D/window/GPU).
    let playable_claim = false;

    let host_path_ok = host_constructed
        && skirmish_config_ok
        && menu_config_ok
        && frames_ok
        && presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && dual_tick_presentation_ok
        && screen_skirmish_ok
        && control_bar_layout_ok
        && map_requirement_ok;

    // Limited claim: headless production host path is operational end-to-end.
    // Requires dual-tick presentation + multi-consumer selection + shell→InGame +
    // ControlBar.wnd ensure. Still not windowed W3D play (playable_claim stays false).
    let shell_host_playable_ok = host_path_ok;

    let status = if host_path_ok {
        "success".into()
    } else {
        "partial".into()
    };

    ShellSmokeResult {
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        presentation_ok,
        dual_tick_presentation_ok,
        dual_tick_counters_ok,
        hud_selection_ok,
        minimap_fow_presentation_ok,
        laser_segment_upload_ok,
        multi_beam_soft_edge_ok,
        laser_presentation_residual_ok,
        floating_text_layout_ok,
        floating_text_vanish_ok,
        world_anim_presentation_ok,
        world_anim_layout_ok,
        world_anim_fade_ok,
        anim2d_frame_ok,
        anim2d_collection_residual_ok,
        translate_copy_residual_ok,
        game_text_caption_ok,
        game_text_csf_str_ok,
        display_string_measure_ok,
        rng_stream_residual_ok,
        mesh_asset_residual_ok,
        rng_residual_pack_ok,
        special_power_wave72_residual_ok,
        special_power_wave73_residual_ok,
        special_power_wave76_residual_ok,
        paradrop_wave76_residual_ok,
        control_bar_wave76_residual_ok,
        graphics_wave76_residual_ok,
        spectre_orbit_decal_presentation_ok,
        special_power_wave77_residual_ok,
        fow_residual_pack_ok,
        ground_height_presentation_ok,
        weapon_store_seed_residual_ok,
        ai_skirmish_residual_ok,
        special_power_wave78_residual_ok,
        cluster_mines_wave78_residual_ok,
        gps_scrambler_wave78_residual_ok,
        cash_bounty_wave78_residual_ok,
        minimap_residual_pack_ok,
        selection_hud_residual_pack_ok,
        input_residual_pack_ok,
        drawable_residual_fields_ok,
        unit_training_wave79_residual_ok,
        upgrades_cost_time_application_ok,
        screen_skirmish_ok,
        control_bar_layout_ok,
        control_bar_path_resolved,
        control_bar_wnd_validated,
        control_bar_window_loaded,
        control_bar_window_count,
        selection_consumers_ok,
        shell_host_playable_ok,
        playable_claim,
        status,
        detail: format!(
            "host={host_constructed} cfg={skirmish_config_ok} menu_cfg={menu_config_ok} map_res={map_resolved} map_load={map_loaded} frames={frames_advanced} pres={presentation_ok} dual_tick={dual_tick_presentation_ok} dual_tick_ctr={dual_tick_counters_ok} hud_sel={hud_selection_ok} sel_consumers={selection_consumers_ok} minimap_fow={minimap_fow_presentation_ok} laser_upload={laser_segment_upload_ok} multi_beam={multi_beam_soft_edge_ok} laser_pres={laser_presentation_residual_ok} floating_text={floating_text_layout_ok} ft_vanish={floating_text_vanish_ok} world_anim={world_anim_presentation_ok} world_anim_layout={world_anim_layout_ok} wa_fade={world_anim_fade_ok} anim2d={anim2d_frame_ok} anim2d_col={anim2d_collection_residual_ok} translate_copy={translate_copy_residual_ok} game_text={game_text_caption_ok} csf_str={game_text_csf_str_ok} ds_measure={display_string_measure_ok} rng={rng_stream_residual_ok} mesh={mesh_asset_residual_ok} rng_pack={rng_residual_pack_ok} sp72={special_power_wave72_residual_ok} sp73={special_power_wave73_residual_ok} sp76={special_power_wave76_residual_ok} paradrop76={paradrop_wave76_residual_ok} cb76={control_bar_wave76_residual_ok} gfx76={graphics_wave76_residual_ok} spectre_decal={spectre_orbit_decal_presentation_ok} sp77={special_power_wave77_residual_ok} fow77={fow_residual_pack_ok} gh77={ground_height_presentation_ok} weapon77={weapon_store_seed_residual_ok} ai77={ai_skirmish_residual_ok} sp78={special_power_wave78_residual_ok} cluster78={cluster_mines_wave78_residual_ok} gps78={gps_scrambler_wave78_residual_ok} cash78={cash_bounty_wave78_residual_ok} minimap79={minimap_residual_pack_ok} sel79={selection_hud_residual_pack_ok} input79={input_residual_pack_ok} draw79={drawable_residual_fields_ok} train79={unit_training_wave79_residual_ok} upg79={upgrades_cost_time_application_ok} screen={screen_skirmish_ok} control_bar={control_bar_layout_ok} cb_path={control_bar_path_resolved} cb_valid={control_bar_wnd_validated} cb_loaded={control_bar_window_loaded} cb_windows={control_bar_window_count} shell_host_playable_ok={shell_host_playable_ok} playable_claim={playable_claim} {layout_report}"
        ),
    }
}

pub fn format_shell_smoke_report(r: &ShellSmokeResult) -> String {
    format!("shell_smoke status={} detail={}", r.status, r.detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn host_smoke_applies_skirmish_and_advances_frames() {
        let r = run_shell_smoke(8);
        assert!(r.host_constructed, "host only after apply: {}", r.detail);
        assert!(r.skirmish_config_ok, "{}", r.detail);
        assert!(r.menu_config_ok, "{}", r.detail);
        assert!(r.frames_advanced > 0, "{}", r.detail);
        assert!(r.hud_selection_ok, "HUD selection residual: {}", r.detail);
        assert!(
            r.dual_tick_presentation_ok,
            "dual-tick presentation residual: {}",
            r.detail
        );
        assert!(
            r.dual_tick_counters_ok,
            "dual-tick residual counters: {}",
            r.detail
        );
        assert!(
            r.minimap_fow_presentation_ok,
            "minimap FOW presentation residual: {}",
            r.detail
        );
        assert!(
            r.laser_segment_upload_ok,
            "laser segment CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.multi_beam_soft_edge_ok,
            "multi-beam soft-edge residual: {}",
            r.detail
        );
        assert!(
            r.laser_presentation_residual_ok,
            "laser presentation residual: {}",
            r.detail
        );
        assert!(
            r.floating_text_layout_ok,
            "floating text CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.floating_text_vanish_ok,
            "floating text vanish-rate residual: {}",
            r.detail
        );
        assert!(
            r.game_text_caption_ok,
            "GUI:AddCash caption residual: {}",
            r.detail
        );
        assert!(
            r.game_text_csf_str_ok,
            "CSF/STR GameText residual: {}",
            r.detail
        );
        assert!(
            r.display_string_measure_ok,
            "DisplayString measure residual: {}",
            r.detail
        );
        assert!(
            r.translate_copy_residual_ok,
            "translate_copy residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_layout_ok,
            "world anim CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_fade_ok,
            "world anim fade residual: {}",
            r.detail
        );
        assert!(
            r.anim2d_frame_ok,
            "Anim2D frame advance residual: {}",
            r.detail
        );
        assert!(
            r.anim2d_collection_residual_ok,
            "Anim2DCollection residual: {}",
            r.detail
        );
        assert!(
            r.rng_stream_residual_ok,
            "RNG stream residual: {}",
            r.detail
        );
        assert!(
            r.mesh_asset_residual_ok,
            "mesh asset residual: {}",
            r.detail
        );
        assert!(
            r.rng_residual_pack_ok,
            "RNG residual pack wave72: {}",
            r.detail
        );
        assert!(
            r.special_power_wave72_residual_ok,
            "special power residual pack wave72: {}",
            r.detail
        );
        assert!(
            r.special_power_wave73_residual_ok,
            "special power residual pack wave73: {}",
            r.detail
        );
        assert!(
            r.special_power_wave76_residual_ok,
            "special power residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.paradrop_wave76_residual_ok,
            "paradrop science-tier residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.control_bar_wave76_residual_ok,
            "control bar residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.graphics_wave76_residual_ok,
            "graphics residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.spectre_orbit_decal_presentation_ok,
            "spectre orbit decal presentation residual: {}",
            r.detail
        );
        assert!(
            r.special_power_wave77_residual_ok,
            "special power audio residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.fow_residual_pack_ok,
            "FOW residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.ground_height_presentation_ok,
            "ground height presentation residual wave77: {}",
            r.detail
        );
        assert!(
            r.weapon_store_seed_residual_ok,
            "weapon store seed residual wave77: {}",
            r.detail
        );
        assert!(
            r.ai_skirmish_residual_ok,
            "AI skirmish residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.special_power_wave78_residual_ok,
            "special power residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.cluster_mines_wave78_residual_ok,
            "cluster mines residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.gps_scrambler_wave78_residual_ok,
            "GPS scrambler residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.cash_bounty_wave78_residual_ok,
            "cash bounty residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.minimap_residual_pack_ok,
            "minimap residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.selection_hud_residual_pack_ok,
            "selection HUD residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.input_residual_pack_ok,
            "input residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.drawable_residual_fields_ok,
            "drawable residual fields wave79: {}",
            r.detail
        );
        assert!(
            r.unit_training_wave79_residual_ok,
            "unit training residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.upgrades_cost_time_application_ok,
            "upgrades cost/time application wave79: {}",
            r.detail
        );
        assert!(
            r.world_anim_presentation_ok,
            "world anim presentation residual: {}",
            r.detail
        );
        assert!(
            r.control_bar_layout_ok,
            "ControlBar.wnd ensure residual: {}",
            r.detail
        );
        assert!(
            r.selection_consumers_ok,
            "multi-consumer selection panel residual: {}",
            r.detail
        );
        // When WindowZH is present, path+validate honesty must be true; prefer
        // headless WindowManager load (not required for CI without assets).
        if r.control_bar_path_resolved {
            assert!(
                r.control_bar_wnd_validated,
                "ControlBar structural validate residual: {}",
                r.detail
            );
            #[cfg(feature = "game_client")]
            if r.control_bar_window_loaded {
                assert!(
                    r.control_bar_window_count > 0,
                    "WindowManager load must materialise windows: {}",
                    r.detail
                );
            }
        } else {
            assert!(
                !r.control_bar_window_loaded && r.control_bar_window_count == 0,
                "missing assets must not claim window load: {}",
                r.detail
            );
        }
        assert!(
            r.screen_skirmish_ok,
            "shell→InGame screen residual: {}",
            r.detail
        );
        // Limited host claim when path is fully operational; never retail W3D claim.
        assert!(
            r.shell_host_playable_ok,
            "shell_host_playable_ok for successful headless host path: {}",
            r.detail
        );
        assert!(!r.playable_claim, "headless smoke must not claim retail playable");
        assert_eq!(r.status, "success", "{}", r.detail);
        assert_eq!(
            r.shell_host_playable_ok,
            r.status == "success",
            "shell_host_playable_ok must track success without overclaiming playable_claim"
        );
    }

    #[test]
    fn shell_host_playable_ok_never_implies_retail_playable_claim() {
        let r = run_shell_smoke(4);
        // Documented honesty contract: limited host flag is independent of retail claim.
        if r.shell_host_playable_ok {
            assert!(
                !r.playable_claim,
                "shell_host_playable_ok must never flip playable_claim"
            );
        }
        assert!(!r.playable_claim);
    }

    #[test]
    fn presentation_carries_transform_health_team_model() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresFields");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("SmokeUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SmokeUnit".into(), t);
        let id = logic
            .create_object("SmokeUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("unit");
        logic.update();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in presentation");
        assert_eq!(obj.team, Team::USA);
        assert!((obj.position.x - 3.0).abs() < 0.01);
        assert!(obj.health_current > 0.0);
        assert_eq!(obj.health_max, 50.0);
        assert_eq!(obj.model_key.as_deref(), Some("SmokeUnit"));
        assert!(!obj.destroyed);
    }

    #[test]
    fn dual_tick_after_map_load_seeds_hud_selection_health() {
        // Residual closed by this change: after skirmish config + (optional) map load,
        // dual-tick presentation must put selection health on GameHUD.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ShellHudSel");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("ShellSelUnit");
        t.set_health(64.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ShellSelUnit".into(), t);
        let id = logic
            .create_object("ShellSelUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }

        // Seed like start_game_from_ui before first logic frame.
        let mut hud = GameHUD::new();
        let seed = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        assert!(
            seed.alive_object_count() >= 1,
            "seed presentation must see map/host units"
        );
        assert!(
            hud.selected_unit_ids().contains(&id),
            "seed apply must set HUD selection"
        );

        logic.update();
        let post = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        let info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("dual-tick HUD selection health");
        assert!(
            (info.health_current - 64.0).abs() < 0.01,
            "health from presentation after dual-tick: {}",
            info.health_current
        );
        assert!(
            hud.selection_panel().has_positive_health(),
            "ControlBar selection panel health after dual-tick"
        );
        assert!(
            (hud.selection_panel().health_current - 64.0).abs() < 0.01,
            "selection panel HP from presentation: {}",
            hud.selection_panel().health_current
        );
        assert_eq!(post.frame.0, logic.get_frame());
        assert!(!post.hud_minimap_units().is_empty());

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            post.apply_to_control_bar(&mut bar);
            let (hp, _) = bar
                .selection_panel_health()
                .expect("ControlBar health from dual-tick presentation");
            assert!((hp - 64.0).abs() < 0.01, "ControlBar HP {hp}");
        }
    }
}
