//! Presentation residual packs (laser / projectile / move / attack / floating-text / world-anim).

#![allow(unused_imports)]

use super::imports::*;
use crate::presentation_frame::PresentationFrame;

pub(super) struct PresentationResiduals {
    pub minimap_fow_presentation_ok: bool,
    pub laser_segment_upload_ok: bool,
    pub projectile_segment_upload_ok: bool,
    pub move_line_upload_ok: bool,
    pub attack_line_upload_ok: bool,
    pub multi_beam_soft_edge_ok: bool,
    pub laser_presentation_residual_ok: bool,
    pub floating_text_layout_ok: bool,
    pub floating_text_vanish_ok: bool,
    pub game_text_caption_ok: bool,
    pub display_string_measure_ok: bool,
    pub game_text_csf_str_ok: bool,
    pub translate_copy_residual_ok: bool,
    pub world_anim_presentation_ok: bool,
    pub world_anim_layout_ok: bool,
    pub world_anim_fade_ok: bool,
    pub anim2d_frame_ok: bool,
    pub anim2d_collection_residual_ok: bool,
    pub rng_stream_residual_ok: bool,
}

pub(super) fn evaluate_presentation_residuals(
    pres: &PresentationFrame,
    presentation_ok: bool,
) -> PresentationResiduals {
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
    // Projectile trail CPU pack residual from presentation freeze.
    let proj_empty = crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::empty();
    let mut proj_logic = crate::game_logic::GameLogic::new();
    let _ = proj_logic.combat_system_mut().fire_projectile(
        glam::Vec3::ZERO,
        glam::Vec3::new(10.0, 0.0, 0.0),
        &crate::game_logic::Weapon::default(),
        crate::game_logic::ObjectId(1),
        None,
        50.0,
    );
    let proj_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&proj_logic, 0);
    let proj_pack =
        crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::pack_from_presentation(
            &proj_pres,
        );
    let projectile_segment_upload_ok = proj_empty.honesty.cpu_pack_ok
        && proj_empty.is_upload_ready()
        && proj_pack.honesty.has_geometry
        && proj_pack.honesty.projectiles_packed >= 1
        && proj_pack.is_upload_ready();
    let move_empty = crate::graphics::move_line_upload::MoveLineUpload::empty();
    let mut move_logic = crate::game_logic::GameLogic::new();
    {
        let mut t = crate::game_logic::ThingTemplate::new("ShellMoveLineU");
        t.set_health(20.0);
        t.add_kind_of(crate::game_logic::KindOf::Infantry);
        move_logic.templates.insert("ShellMoveLineU".into(), t);
        if let Some(id) = move_logic.create_object(
            "ShellMoveLineU",
            crate::game_logic::Team::USA,
            glam::Vec3::ZERO,
        ) {
            if let Some(obj) = move_logic.host_object_mut(id) {
                obj.movement.target_position = Some(glam::Vec3::new(5.0, 0.0, 5.0));
            }
        }
    }
    let move_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&move_logic, 0);
    let move_pack =
        crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(&move_pres);
    let move_line_upload_ok = move_empty.honesty.cpu_pack_ok
        && move_empty.is_upload_ready()
        && move_pack.honesty.has_geometry
        && move_pack.honesty.lines_packed >= 1
        && move_pack.is_upload_ready();
    let atk_empty = crate::graphics::attack_line_upload::AttackLineUpload::empty();
    let mut atk_logic = crate::game_logic::GameLogic::new();
    {
        for (name, pos) in [
            ("ShellAtkA", glam::Vec3::ZERO),
            ("ShellAtkB", glam::Vec3::new(8.0, 0.0, 0.0)),
        ] {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            t.set_health(20.0);
            t.add_kind_of(crate::game_logic::KindOf::Infantry);
            atk_logic.templates.insert(name.into(), t);
            let _ = atk_logic.create_object(name, crate::game_logic::Team::USA, pos);
        }
        let ids: Vec<_> = atk_logic.host_objects().keys().copied().collect();
        if ids.len() >= 2 {
            if let Some(obj) = atk_logic.host_object_mut(ids[0]) {
                obj.target = Some(ids[1]);
            }
        }
    }
    let atk_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&atk_logic, 0);
    let atk_pack =
        crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(&atk_pres);
    let attack_line_upload_ok = atk_empty.honesty.cpu_pack_ok
        && atk_empty.is_upload_ready()
        && atk_pack.honesty.has_geometry
        && atk_pack.honesty.lines_packed >= 1
        && atk_pack.is_upload_ready();
    // OrbitalLaser multi-beam soft-edge: presentation residual fields → CPU pack.
    let orbital = PresentationLaserBeam::synthetic_orbital_soft_edge(pres.frame.0);
    let se = orbital.soft_edge.unwrap_or(PRESENTATION_ORBITAL_SOFT_EDGE);
    let (mb_start, mb_end, mb_elapsed, mb_width) = se.pack_endpoints(orbital.from, orbital.to, 1.0);
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
    ft_synth_frame.floating_texts =
        vec![PresentationFloatingText::synthetic_cash(100, pres.frame.0)];
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

    PresentationResiduals {
        minimap_fow_presentation_ok,
        laser_segment_upload_ok,
        projectile_segment_upload_ok,
        move_line_upload_ok,
        attack_line_upload_ok,
        multi_beam_soft_edge_ok,
        laser_presentation_residual_ok,
        floating_text_layout_ok,
        floating_text_vanish_ok,
        game_text_caption_ok,
        display_string_measure_ok,
        game_text_csf_str_ok,
        translate_copy_residual_ok,
        world_anim_presentation_ok,
        world_anim_layout_ok,
        world_anim_fade_ok,
        anim2d_frame_ok,
        anim2d_collection_residual_ok,
        rng_stream_residual_ok,
    }
}
