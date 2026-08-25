#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// One C++ `InGameUI::m_idleWorkers` entry projected onto the host-owned
/// presentation frame.  The worker id drives C++'s first/next/wrap choice;
/// selection and camera may target its container instead.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IdleWorkerSelectionTarget {
    worker_id: ObjectId,
    selection_id: ObjectId,
    focus_position: glam::Vec3,
}

/// C++ `ALLOW_SHRUBBERY_TARGET` / `ALLOW_MINE_TARGET` (`Command.h`).
const CMD_ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
const CMD_ALLOW_MINE_TARGET: u32 = 0x0000_0800;

/// Live-host analog of leftover `ContextPickProfile` mine/shrubbery/force-attack bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HostContextPickProfile {
    include_mines: bool,
    include_shrubbery: bool,
    include_force_attackable: bool,
}

/// C++ `getPickTypesForContext` / `getPickTypesForCurrentSelection`
/// (`SelectionInfo.cpp:227-295`). Always `PICK_TYPE_SELECTABLE`; force-attack
/// adds `PICK_TYPE_FORCEATTACKABLE`. An armed GUI command owns the extra
/// mine/shrubbery bits; otherwise force-attack + a `DAMAGE_FLAME` weapon adds
/// shrubbery. Disarm no longer auto-picks mines.
pub(super) fn host_context_pick_profile(
    force_attack_mode: bool,
    armed_gui_command_options: Option<u32>,
    selection_has_flame: bool,
) -> HostContextPickProfile {
    let mut profile = HostContextPickProfile::default();
    if force_attack_mode {
        profile.include_force_attackable = true;
    }
    if let Some(options) = armed_gui_command_options {
        if options & CMD_ALLOW_MINE_TARGET != 0 {
            profile.include_mines = true;
        }
        if options & CMD_ALLOW_SHRUBBERY_TARGET != 0 {
            profile.include_shrubbery = true;
        }
    } else if force_attack_mode && selection_has_flame {
        profile.include_shrubbery = true;
    }
    profile
}

fn presentation_is_mine_pick(o: &crate::presentation_frame::RenderableObject) -> bool {
    use crate::game_logic::KindOf;
    o.has_mine
        || crate::presentation_frame::PresentationFrame::object_has_kind(o, KindOf::Mine)
        || crate::presentation_frame::PresentationFrame::object_has_kind(o, KindOf::DemoTrap)
        || crate::game_logic::host_car_bomb::object_definition_has_kind(&o.template_name, "MINE")
        || crate::game_logic::host_car_bomb::object_definition_has_kind(
            &o.template_name,
            "DEMOTRAP",
        )
}

fn presentation_is_shrubbery_pick(o: &crate::presentation_frame::RenderableObject) -> bool {
    crate::game_logic::host_car_bomb::object_definition_has_kind(&o.template_name, "SHRUBBERY")
}

fn presentation_is_force_attackable_pick(o: &crate::presentation_frame::RenderableObject) -> bool {
    use crate::game_logic::KindOf;
    o.is_force_attackable
        || crate::presentation_frame::PresentationFrame::object_has_kind(o, KindOf::ForceAttackable)
        || crate::game_logic::host_car_bomb::object_definition_has_kind(
            &o.template_name,
            "FORCEATTACKABLE",
        )
}

fn profile_widens_presentation(
    o: &crate::presentation_frame::RenderableObject,
    profile: HostContextPickProfile,
) -> bool {
    (profile.include_mines && presentation_is_mine_pick(o))
        || (profile.include_shrubbery && presentation_is_shrubbery_pick(o))
        || (profile.include_force_attackable && presentation_is_force_attackable_pick(o))
}

fn profile_has_widened_bits(profile: HostContextPickProfile) -> bool {
    profile.include_mines || profile.include_shrubbery || profile.include_force_attackable
}

fn presentation_object_has_flame_weapon(o: &crate::presentation_frame::RenderableObject) -> bool {
    let Some(name) = crate::game_logic::primary_weapon_name_for_unit(&o.template_name) else {
        return false;
    };
    crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(name)
        == crate::game_logic::combat::DamageType::Flame
}

/// Nearest mine/shrubbery/force-attackable under the cursor when the pick profile widens.
pub(super) fn pick_widened_context_target(
    frame: &crate::presentation_frame::PresentationFrame,
    position: glam::Vec3,
    player_team: Option<crate::game_logic::Team>,
    base_selection_radius: f32,
    profile: HostContextPickProfile,
) -> Option<ObjectId> {
    if !profile_has_widened_bits(profile) {
        return None;
    }

    let mut best: Option<(ObjectId, f32)> = None;
    for o in &frame.objects {
        if o.destroyed {
            continue;
        }
        if frame.box_pick_hides_non_local(o) {
            continue;
        }
        if !profile_widens_presentation(o, profile) {
            continue;
        }

        let distance = o.position.distance(position);
        let radius = base_selection_radius.max(o.selection_radius);
        if distance > radius {
            continue;
        }
        if best.is_none_or(|(_, best_d)| distance < best_d) {
            best = Some((o.id, distance));
        }
    }
    best.map(|(id, _)| id)
}

fn pick_widened_context_target_along_ray(
    frame: &crate::presentation_frame::PresentationFrame,
    ray_start: glam::Vec3,
    ray_end: glam::Vec3,
    player_team: Option<crate::game_logic::Team>,
    profile: HostContextPickProfile,
) -> Option<ObjectId> {
    if !profile_has_widened_bits(profile) {
        return None;
    }
    let ray_dir = ray_end - ray_start;
    let mut best: Option<(ObjectId, f32)> = None;
    for o in &frame.objects {
        if o.destroyed {
            continue;
        }
        if frame.box_pick_hides_non_local(o) {
            continue;
        }
        if !profile_widens_presentation(o, profile) {
            continue;
        }

        let radius =
            crate::pick_ray::presentation_mesh_pick_radius(o.selection_radius, o.health_box_width);
        let Some(t) = crate::pick_ray::ray_sphere_hit_t(ray_start, ray_dir, o.position, radius)
        else {
            continue;
        };
        if t > 1.0 {
            continue;
        }
        if best.is_none_or(|(_, best_t)| t < best_t) {
            best = Some((o.id, t));
        }
    }
    best.map(|(id, _)| id)
}

fn closer_presentation_pick_along_ray(
    frame: &crate::presentation_frame::PresentationFrame,
    ray_start: glam::Vec3,
    ray_end: glam::Vec3,
    standard: Option<ObjectId>,
    extra: Option<ObjectId>,
) -> Option<ObjectId> {
    let ray_dir = ray_end - ray_start;
    let hit_t = |id: ObjectId| {
        frame.objects.iter().find(|o| o.id == id).and_then(|o| {
            let radius = crate::pick_ray::presentation_mesh_pick_radius(
                o.selection_radius,
                o.health_box_width,
            );
            crate::pick_ray::ray_sphere_hit_t(ray_start, ray_dir, o.position, radius)
        })
    };
    match (standard, extra) {
        (Some(s), Some(e)) if s != e => {
            let st = hit_t(s).unwrap_or(f32::MAX);
            let et = hit_t(e).unwrap_or(f32::MAX);
            if et < st { Some(e) } else { Some(s) }
        }
        (s, e) => s.or(e),
    }
}

fn project_world_to_screen(
    view_projection: glam::Mat4,
    position: glam::Vec3,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<glam::Vec2> {
    let clip = view_projection * position.extend(1.0);
    if !clip.is_finite() || clip.w <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() || !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    let screen = glam::Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport_width,
        (1.0 - ndc.y) * 0.5 * viewport_height,
    );
    screen.is_finite().then_some(screen)
}

#[cfg(feature = "game_client")]
fn opaque_see_thru_chain(
    mut window: Option<std::rc::Rc<std::cell::RefCell<game_client::gui::GameWindow>>>,
) -> Vec<bool> {
    let mut chain = Vec::new();
    while let Some(current) = window {
        let guard = current.borrow();
        chain.push(
            guard
                .get_status()
                .contains(game_client::gui::WindowStatus::SEE_THRU),
        );
        window = guard.get_parent();
    }
    chain
}

pub(super) fn closer_presentation_pick(
    frame: &crate::presentation_frame::PresentationFrame,
    position: glam::Vec3,
    standard: Option<ObjectId>,
    extra: Option<ObjectId>,
) -> Option<ObjectId> {
    match (standard, extra) {
        (Some(s), Some(e)) if s != e => {
            let sd = frame
                .objects
                .iter()
                .find(|o| o.id == s)
                .map(|o| o.position.distance(position))
                .unwrap_or(f32::MAX);
            let ed = frame
                .objects
                .iter()
                .find(|o| o.id == e)
                .map(|o| o.position.distance(position))
                .unwrap_or(f32::MAX);
            if ed < sd { Some(e) } else { Some(s) }
        }
        (s, e) => s.or(e),
    }
}

/// Mirror `InGameUI::selectNextIdleWorker`: only exactly one currently
/// selected idle worker advances; any empty, multi, or unrelated selection
/// starts from the first worker.
fn select_next_idle_worker_target(
    targets: &[IdleWorkerSelectionTarget],
    selected: &[ObjectId],
) -> Option<IdleWorkerSelectionTarget> {
    let first = *targets.first()?;
    if selected.len() != 1 {
        return Some(first);
    }

    let selected_worker = selected[0];
    let Some(index) = targets
        .iter()
        .position(|target| target.worker_id == selected_worker)
    else {
        return Some(first);
    };
    Some(targets[(index + 1) % targets.len()])
}

/// Build the stable local idle-worker list from frozen host data without
/// reading legacy GameLogic globals.  C++ selects a containing object when an
/// idle worker is contained, so validate and retain that selectable container
/// as the target while preserving the worker id for cycle order.
fn idle_worker_selection_targets_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
    team: crate::game_logic::Team,
) -> Vec<IdleWorkerSelectionTarget> {
    use crate::presentation_frame::PresentationFrame;
    use crate::unit_control::UnitControlSystem;

    let mut targets: Vec<_> = frame
        .objects
        .iter()
        .filter_map(|worker| {
            let idle_worker = worker.team == team
                && !worker.destroyed
                && !worker.sold
                && PresentationFrame::presentation_is_worker_like(worker)
                && worker.move_destination.is_none()
                && worker.attack_target.is_none()
                && !worker.under_construction
                && worker.ai_state_ordinal == 0;
            idle_worker.then_some(())?;

            let target = match worker.contained_by {
                Some(container_id) => frame
                    .objects
                    .iter()
                    .find(|object| object.id == container_id)?,
                None => worker,
            };
            (target.team == team && UnitControlSystem::presentation_is_selectable(target))
                .then_some(())?;

            Some(IdleWorkerSelectionTarget {
                worker_id: worker.id,
                selection_id: target.id,
                focus_position: target.position,
            })
        })
        .collect();
    // The host presentation has no live `ObjectList` ownership.  Preserve a
    // stable deterministic order for C++ first/next/wrap semantics.
    targets.sort_by_key(|target| target.worker_id.0);
    targets
}

impl CnCGameEngine {
    pub(super) fn host_selection_has_flame_weapon(&self) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        let selected = self.ui_selected_ids(self.current_player_id);
        for id in selected {
            if let Some(obj) = self.game_logic.host_object(id) {
                if let Some(name) = obj.get_template().primary_weapon_name.as_deref() {
                    if crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
                        name,
                    ) == crate::game_logic::combat::DamageType::Flame
                    {
                        return true;
                    }
                }
            }
            if let Some(o) = frame.objects.iter().find(|o| o.id == id) {
                if presentation_object_has_flame_weapon(o) {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn host_find_object_at_position(
        &self,
        position: glam::Vec3,
        command_context: bool,
    ) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 20.0;
        let frame = self.last_presentation_frame.as_ref()?;
        let player_team = Some(frame.local_team());
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;
        let force_attack_mode = self.keys_pressed.contains(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Control,
        ));
        let profile = host_context_pick_profile(
            force_attack_mode,
            self.host_armed_gui_command_options(),
            self.host_selection_has_flame_weapon(),
        );
        let standard =
            crate::unit_control::UnitControlSystem::pick_object_id_at_world_from_presentation(
                frame,
                position,
                player_team,
                prioritize_enemy_targets,
                BASE_SELECTION_RADIUS,
            );
        let extra = pick_widened_context_target(
            frame,
            position,
            player_team,
            BASE_SELECTION_RADIUS,
            profile,
        );
        closer_presentation_pick(frame, position, standard, extra)
            .map(|id| self.remap_ignored_in_gui_pick(frame, id))
    }

    /// C++ `W3DView::pickDrawable` + point `iterateDrawablesInRegion`.
    pub(super) fn host_pick_object_at_cursor(&self, command_context: bool) -> Option<ObjectId> {
        self.host_pick_object_at_cursor_ex(command_context, false)
    }

    /// C++ `SelectionXlat.cpp:429` mouseover hardcodes `getPickTypesForContext(true)`.
    pub(super) fn host_pick_hover_object_at_cursor(&self) -> Option<ObjectId> {
        self.host_pick_object_at_cursor_ex(true, true)
    }

    fn host_pick_object_at_cursor_ex(
        &self,
        command_context: bool,
        hover_force_attackable: bool,
    ) -> Option<ObjectId> {
        if self.host_cursor_blocked_by_opaque_window() {
            return None;
        }
        let frame = self.last_presentation_frame.as_ref()?;
        let (view_w, view_h) = self.tactical_viewport_size();
        let (ray_start, ray_end) = super::mouse::unproject_mouse_ray(
            self.view_matrix,
            self.projection_matrix,
            self.mouse_position,
            view_w,
            view_h,
        )?;
        let player_team = Some(frame.local_team());
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;
        let force_attack_mode = self.keys_pressed.contains(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Control,
        ));
        let mut profile = host_context_pick_profile(
            force_attack_mode,
            self.host_armed_gui_command_options(),
            self.host_selection_has_flame_weapon(),
        );
        if hover_force_attackable {
            profile.include_force_attackable = true;
        }
        let standard = crate::pick_ray::pick_object_id_along_camera_ray(
            frame,
            ray_start,
            ray_end,
            player_team,
            prioritize_enemy_targets,
        );
        let extra =
            pick_widened_context_target_along_ray(frame, ray_start, ray_end, player_team, profile);
        closer_presentation_pick_along_ray(frame, ray_start, ray_end, standard, extra)
            .filter(|&id| !self.host_object_id_blocked_by_opaque_hud(id))
            .map(|id| self.remap_ignored_in_gui_pick(frame, id))
    }

    fn remap_ignored_in_gui_pick(
        &self,
        frame: &crate::presentation_frame::PresentationFrame,
        id: ObjectId,
    ) -> ObjectId {
        // C++ InGameUI.cpp:2265-2278 — IGNORED_IN_GUI remaps to slaver/nexus.
        frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| {
                if crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::IgnoredInGui,
                ) {
                    o.producer_id
                } else {
                    None
                }
            })
            .unwrap_or(id)
    }

    fn host_cursor_blocked_by_opaque_window(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            let x = self.mouse_position.0 as i32;
            let y = self.mouse_position.1 as i32;
            game_client::gui::with_window_manager_ref(|manager| {
                crate::pick_ray::opaque_window_chain_blocks_pick(&opaque_see_thru_chain(
                    manager.get_window_under_cursor(x, y, false),
                ))
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    pub(super) fn host_object_id_blocked_by_opaque_hud(&self, id: ObjectId) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        let Some(object) = frame.objects.iter().find(|o| o.id == id) else {
            return false;
        };
        let (view_w, view_h) = self.tactical_viewport_size();
        let view_projection = self.projection_matrix * self.view_matrix;
        let Some(screen) =
            project_world_to_screen(view_projection, object.position, view_w, view_h)
        else {
            return false;
        };
        #[cfg(feature = "game_client")]
        {
            game_client::gui::with_window_manager_ref(|manager| {
                crate::pick_ray::opaque_window_chain_blocks_pick(&opaque_see_thru_chain(
                    manager.get_window_under_cursor(screen.x as i32, screen.y as i32, false),
                ))
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = screen;
            false
        }
    }

    /// Retail `ControlBar.wnd:ButtonIdleWorker` and
    /// `IdleWorker.wnd:ButtonSelectNextIdleWorker` action.  This deliberately
    /// differs from the broader hotkey worker cycle: C++ considers idle
    /// workers only and always centers the tactical view on the selected
    /// worker (or its container).
    pub(super) fn host_select_next_idle_worker_from_control_bar(&mut self) {
        let (local_player_id, targets) = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            (
                frame.local_player_id,
                idle_worker_selection_targets_from_presentation(frame, frame.local_team()),
            )
        };
        let Some(target) = select_next_idle_worker_target(&targets, &self.selected_objects) else {
            return;
        };

        self.host_set_selection(local_player_id, vec![target.selection_id]);
        self.play_sound_effect(SoundType::Select);
        self.host_center_camera_and_request_focus(target.focus_position);
    }

    /// Retail SELECT_NEXT/PREV_UNIT residual.
    pub(super) fn cycle_friendly_selection(&mut self, delta: i32) {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let all: Vec<ObjectId> = frame
            .objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && o.team == team
                    && o.is_mobile
                    && o.contained_by.is_none()
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
            .collect();
        if all.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            // C++ CommandXlat.cpp:2396 — non-local inspect selection is a no-op.
            let Some(idx) = all.iter().position(|id| *id == current) else {
                return;
            };
            let n = all.len() as i32;
            let i = (idx as i32 + delta).rem_euclid(n) as usize;
            all[i]
        } else if delta >= 0 {
            all[0]
        } else {
            all[all.len() - 1]
        };

        let look = frame
            .objects
            .iter()
            .find(|o| o.id == next)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        if let Some(pos) = look {
            self.host_player_look_at(pos);
        }
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail SELECT_NEXT/PREV_WORKER — KINDOF_DOZER only, lookAt the chosen dozer.
    pub(super) fn cycle_friendly_worker_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let workers: Vec<(ObjectId, glam::Vec3)> = frame
            .objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && o.team == team
                    && o.contained_by.is_none()
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                    && crate::presentation_frame::PresentationFrame::object_has_kind(
                        o,
                        crate::game_logic::KindOf::Dozer,
                    )
                    && (delta >= 0 || o.is_mobile)
            })
            .map(|o| (o.id, o.position))
            .collect();
        if workers.is_empty() {
            return;
        }

        // Ascending ObjectId = oldest first. C++ NEXT from nothing picks oldest;
        // NEXT from current walks toward newer (+1).
        let next = if let Some(current) = self.selected_objects.first().copied() {
            let Some(idx) = workers.iter().position(|(id, _)| *id == current) else {
                return;
            };
            let n = workers.len() as i32;
            let i = (idx as i32 + delta).rem_euclid(n) as usize;
            workers[i]
        } else if delta >= 0 {
            workers[0]
        } else {
            workers[workers.len() - 1]
        };

        self.host_set_selection(self.current_player_id, vec![next.0]);
        self.host_player_look_at(next.1);
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail-ish SELECT_NEXT/PREV_STRUCTURE residual.
    pub(super) fn cycle_friendly_structure_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut structures: Vec<ObjectId> = frame
            .objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && o.team == team
                    && o.is_structure
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
            .collect();
        structures.sort_by_key(|id| id.0);
        if structures.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            structures
                .iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = structures.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    structures[i]
                })
                .unwrap_or(structures[0])
        } else if delta >= 0 {
            structures[0]
        } else {
            structures[structures.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            self.host_player_look_at(pos);
        }
    }
    /// Cycle damaged friendly structures residual (for repair response).

    /// Cycle unfinished friendly construction residual (Ctrl+Alt+Home/End).

    /// Resume unfinished construction with selected dozers residual (Alt+E).
    /// Wave 612: via `host_resume_selected_construction`.
    pub(super) fn resume_selected_construction(&mut self) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_resume_selected_construction()
    }

    /// Resume unfinished construction with selected dozers residual (Alt+E).
    pub(super) fn host_resume_selected_construction(&mut self) {
        // Wave 612: host residual helper.
        let player_id = self.current_player_id;
        // Wave 226: selection/team via presentation-first helpers.
        let selected = self.ui_selected_ids(player_id);
        let team = self.local_team_for_ui();
        let unfinished: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.objects.iter().any(|o| {
                        o.id == id
                            && o.team == team
                            && !o.destroyed
                            && o.under_construction
                            && !o.sold
                    })
                } else {
                    // Presentation required (no live dual-read).
                    false
                }
            })
            .collect();
        let dozers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.objects.iter().any(|o| {
                        o.id == id
                            && o.team == team
                            && !o.destroyed
                            && crate::presentation_frame::PresentationFrame::presentation_is_worker_like(
                                o,
                            )
                    })
                } else {
                    // Presentation required (no live dual-read).
                    false
                }
            })
            .collect();
        // If only unfinished selected, pick all team dozers idle residual.
        let mut builders = dozers;
        if builders.is_empty() {
            builders = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.alive_selectable_friendly_idle_worker_ids(team)
            } else {
                // Presentation required (no live get_objects dual-read).
                Vec::new()
            };
        }
        let target = unfinished.first().copied().or_else(|| {
            // Fall back to cycled unfinished if selection is dozers only.
            if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame
                    .alive_selectable_friendly_unfinished_ids(team)
                    .into_iter()
                    .next()
            } else {
                // Presentation required (no live get_objects dual-read).
                None
            }
        });
        let Some(target_id) = target else {
            return;
        };
        if builders.is_empty() {
            return;
        }
        self.host_queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::ResumeConstruction { target_id },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: builders,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        self.host_process_commands_with_command_sound();
        // C++ resume construction has no invented HUD toast.
    }

    pub(super) fn cycle_unfinished_construction(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_unfinished_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    pub(super) fn cycle_damaged_structure_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_damaged_structure_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    /// Select all idle friendly combat units residual (Ctrl+I).

    /// C++ `ControlBar::togglePurchaseScience` / leftover `on_generals_button`.
    /// Alt+G and empty-name PurchaseScience open the promotion screen only;
    /// purchase is the clicked science (`GameLogicDispatch` attemptToPurchaseScience).
    pub(super) fn try_purchase_next_generals_science(&mut self) {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheControlBar::toggle_purchase_science();
        }
    }

    /// Cycle idle friendly combat units residual (Ctrl+Alt+, / .).
    pub(super) fn cycle_idle_military_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_idle_military_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    /// Select all friendly units currently repairing residual (Ctrl+Alt+R).
    pub(super) fn select_all_repairing_units(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_repairing_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn select_all_idle_military(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();

        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_idle_military_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
    }

    /// Select all friendly harvesters / supply collectors residual (Ctrl+Shift+I).
    pub(super) fn select_all_harvesters(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();

        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_harvester_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Select idle friendly harvesters residual (Ctrl+Alt+I).
    pub(super) fn select_idle_harvesters(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_idle_harvester_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Cycle construction panel tab residual (`[` / `]`).
    pub(super) fn cycle_construction_tab(&mut self, delta: i32) {
        use crate::ui::ConstructionTab;
        if !self.game_hud.construction_panel.is_visible() {
            return;
        }
        let tabs = [
            ConstructionTab::Buildings,
            ConstructionTab::Infantry,
            ConstructionTab::Vehicles,
            ConstructionTab::Aircraft,
        ];
        let cur = self.game_hud.construction_panel.current_tab();
        let idx = tabs.iter().position(|t| *t == cur).unwrap_or(0) as i32;
        let n = tabs.len() as i32;
        let next = (((idx + delta) % n) + n) % n;
        let tab = tabs[next as usize];
        self.game_hud.construction_panel.force_tab(tab);
    }

    /// Select friendly units near camera (on-screen residual, Ctrl+Alt+A).

    /// Select all friendly structures residual (Ctrl+Alt+S).
    pub(super) fn select_all_friendly_structures(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for o in &frame.objects {
            if o.team == team
                && o.is_structure
                && !o.destroyed
                && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            {
                ids.push(o.id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Adjust guard radius on selected guarding units residual (Alt+[ / ]).

    /// Clear movement path / waypoints on selection residual (Alt+Z).
    pub(super) fn clear_selected_path_waypoints(&mut self) {
        // Wave 225: selection via presentation-first ui_selected_ids; mutation via GameLogic API.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            if self.sticky_waypoint_mode {
                self.sticky_waypoint_mode = false;
            }
            return;
        }
        let mut any = false;
        for id in selected {
            if self.host_clear_unit_movement_path(id) {
                any = true;
            }
        }
        if self.sticky_waypoint_mode {
            self.sticky_waypoint_mode = false;
            any = true;
        }
        if any {
            self.play_sound_effect(SoundType::Command);
        }
    }

    /// Cycle damaged friendly mobile units residual (Ctrl+Alt+Up/Down).
    pub(super) fn cycle_damaged_unit_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_damaged_unit_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    pub(super) fn adjust_selected_guard_radius(&mut self, delta: f32) {
        // Wave 225: selection via presentation-first ui_selected_ids; mutation via GameLogic API.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }
        for id in selected {
            let _ = self.host_adjust_unit_guard_radius(id, delta);
        }
    }

    /// Select all friendly combat units (exclude workers/dozers/supply) residual.

    /// Select all friendly units currently moving residual (Ctrl+Alt+M).

    /// Select all friendly units currently attacking residual (Ctrl+Alt+T).
    pub(super) fn select_all_friendly_attacking(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_attacking_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Issue Stop to all friendly mobile units residual (Ctrl+Alt+S is structures).
    /// Ctrl+Shift+Period residual: stop everything friendly.

    /// Runtime-host residual: ensure at least one local mobile is selected.
    pub(super) fn ensure_host_mobile_selection(&mut self) {
        if !self.selected_objects.is_empty() {
            return;
        }
        // Wave 726: auto-pick first friendly mobile is opt-in only (default fail-closed).
        // Retail commands require a real selection. Vertical-slice smoke already
        // issues select_local_unit / box_select before movement commands.
        // Opt in: GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE=1.
        let allow_auto_select = std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE")
            .is_some_and(|v| {
                let s = v.to_string_lossy();
                !(s.is_empty()
                    || s == "0"
                    || s.eq_ignore_ascii_case("false")
                    || s.eq_ignore_ascii_case("no"))
            });
        if !allow_auto_select {
            return;
        }
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let id = frame
            .alive_selectable_friendly_mobile_ids(team)
            .into_iter()
            .next();
        if let Some(id) = id {
            self.host_set_selection(self.current_player_id, vec![id]);
        }
    }

    /// Wave 611: via `host_stop_all_friendly_units`.
    pub(super) fn stop_all_friendly_units(&mut self) {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_stop_all_friendly_units()
    }

    pub(super) fn host_stop_all_friendly_units(&mut self) {
        // Wave 611: host residual helper.
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids = frame.alive_friendly_stoppable_ids(team);
        if ids.is_empty() {
            return;
        }
        self.host_queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Stop,
            player_id: self.current_player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: ids.clone(),
            modifier_keys: crate::command_system::ModifierKeys {
                ctrl: true,
                shift: true,
                alt: false,
            },
        });
        self.host_process_commands_with_command_sound();
    }

    pub(super) fn select_all_friendly_moving(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_moving_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }
    /// Select friendly transports currently carrying units residual (Ctrl+Alt+J).
    pub(super) fn select_all_occupied_transports(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_occupied_transport_ids(team);
        ids.dedup();
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Toggle attack-order line drawing residual (Ctrl+F4).
    pub(super) fn toggle_attack_lines_hotkey(&mut self) {
        self.show_attack_lines = !self.show_attack_lines;
    }

    /// Toggle movement path line drawing residual (Ctrl+F3).
    pub(super) fn toggle_move_lines_hotkey(&mut self) {
        self.show_move_lines = !self.show_move_lines;
    }

    /// Select structures that currently hold garrisoned units residual (Ctrl+Alt+U).
    pub(super) fn select_all_garrisoned_structures(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        {
            for o in frame.garrisoned_structures() {
                if o.team == team && !o.destroyed {
                    ids.push(o.id);
                }
            }
        };
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn toggle_fps_counter_hotkey(&mut self) {
        self.show_fps = !self.show_fps;
    }

    /// Select all friendly veteran+ units residual (Ctrl+Alt+E).

    /// Cycle non-empty control groups residual (Ctrl+Alt+Left/Right already damaged structures).
    /// Use Ctrl+Shift+Tab residual: next/prev control group.
    pub(super) fn cycle_control_group_selection(&mut self, delta: i32) {
        let mut groups: Vec<u8> = self
            .control_groups
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| *k)
            .collect();
        groups.sort();
        if groups.is_empty() {
            return;
        }
        let current = self
            .last_control_group_select
            .map(|(g, _)| g)
            .and_then(|g| groups.iter().position(|x| *x == g));
        let idx = match current {
            Some(i) => {
                let n = groups.len() as i32;
                ((i as i32 + delta).rem_euclid(n)) as usize
            }
            None => {
                if delta >= 0 {
                    0
                } else {
                    groups.len() - 1
                }
            }
        };
        let group_num = groups[idx];
        let stored = self
            .control_groups
            .get(&group_num)
            .cloned()
            .unwrap_or_default();
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let selection = frame.filter_live_squad_ids(&stored, true);
        if selection.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.last_control_group_select = Some((group_num, Instant::now()));
        self.play_sound_effect(SoundType::Select);
    }

    /// Select all friendly effectively stealthed units residual (Ctrl+Alt+K).
    pub(super) fn select_all_friendly_stealthed(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_stealthed_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn select_all_friendly_veterans(&mut self) {
        use crate::game_logic::VeterancyLevel;
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_veteran_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Select aircraft currently docked/parked residual (Ctrl+Alt+W).
    pub(super) fn select_all_docked_aircraft(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_docked_aircraft_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn toggle_debug_info_hotkey(&mut self) {
        self.show_debug_info = !self.show_debug_info;
    }

    /// Cycle friendly producers with a non-empty queue residual (Ctrl+Alt+P).
    pub(super) fn cycle_busy_producer_selection(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_busy_producer_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    /// Select all friendly units currently guarding residual (Ctrl+Alt+G).

    /// Select all friendly units currently patrolling residual (Ctrl+Alt+Y).
    pub(super) fn select_all_friendly_patrolling(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_patrolling_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Select all friendly units currently gathering residual (Ctrl+Alt+H).
    pub(super) fn select_all_friendly_gathering(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_gathering_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    /// Cycle structures with ready special power residual (Ctrl+Alt+V).
    pub(super) fn cycle_ready_special_power_structure(&mut self, delta: i32) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let mut ids: Vec<ObjectId> = frame.alive_selectable_friendly_ready_special_power_ids(team);
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };

        let cam_pos = frame
            .objects
            .iter()
            .find(|o| o.id == next && !o.destroyed)
            .map(|o| o.position);
        self.host_set_selection(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        if let Some(pos) = cam_pos {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    pub(super) fn select_all_friendly_guarding(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_guarding_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn toggle_health_bars_hotkey(&mut self) {
        self.show_health_bars = !self.show_health_bars;
    }

    pub(super) fn select_all_friendly_combat(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_combat_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn select_all_friendly_on_screen(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let center = self.camera_target;
        // Fail-closed frustum residual: radius scales with zoom.
        let radius = (180.0 * self.camera_zoom.max(0.5)).clamp(120.0, 600.0);
        let selection = frame.alive_selectable_friendly_near(team, center, radius);
        if selection.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// Select friendly dozers/workers currently constructing residual (Ctrl+Alt+B).
    pub(super) fn select_all_constructing_workers(&mut self) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let team = frame.local_team();
        let ids: Vec<crate::game_logic::ObjectId> =
            frame.alive_selectable_friendly_constructing_worker_ids(team);
        if ids.is_empty() {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
    }

    pub(super) fn toggle_camera_follow_selection(&mut self) {
        // Wave 548: presentation freeze owns follow-active residual when installed
        // (`camera_follow_position`; no live `camera_follow_object_id` dual-read).
        // Command authority still writes `set_camera_follow_object` for host follow state.
        // Wave 583: follow-active via presentation_or_boot helper.
        let follow_active = self.presentation_or_boot_camera_follow_active();
        if follow_active {
            self.host_set_camera_follow_object(None);
            return;
        }
        let id = self.ui_selection_seed_id();
        let Some(id) = id else {
            return;
        };
        self.host_set_camera_follow_object(Some(id));
    }

    /// Retail SELECT_ALL (KEY_Q) residual.
    /// C++ `InGameUI::selectAllUnitsByType`: screen pass then map fallback
    /// (`InGameUI.cpp:4877`). CommandXlat excludes DOZER/HARVESTER/IGNORES_SELECT_ALL
    /// and 1.03-incompatible current selection.
    pub(super) fn select_all_friendly_units(&mut self) {
        self.select_all_units_by_type(false);
    }

    fn select_all_units_by_type(&mut self, aircraft_only: bool) {
        let (incompatible, current, candidates, on_screen) = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            let current = self.ui_selected_ids(self.current_player_id);
            let incompatible = current.iter().any(|id| {
                frame.objects.iter().any(|o| {
                    o.id == *id
                        && (crate::presentation_frame::PresentationFrame::is_select_all_disqualified(
                            o,
                        ) || (aircraft_only
                            && !crate::presentation_frame::PresentationFrame::object_has_kind(
                                o,
                                crate::game_logic::KindOf::Aircraft,
                            )))
                })
            });
            let candidates = frame.alive_select_all_unit_ids(frame.local_team(), aircraft_only);
            let (vw, vh) = self.tactical_viewport_size();
            let viewport = glam::Vec2::new(vw, vh);
            let on_screen = frame.filter_ids_on_screen(
                &candidates,
                self.view_matrix,
                self.projection_matrix,
                viewport,
            );
            (incompatible, current, candidates, on_screen)
        };
        // C++ CommandXlat 1.03: deselect incompatible current, then add.
        let mut kept = if incompatible {
            self.host_set_selection(self.current_player_id, Vec::new());
            Vec::new()
        } else {
            current
        };
        // C++ kindOfUnitSelection skips already-selected; AcrossScreen then AcrossMap.
        let already: std::collections::HashSet<_> = kept.iter().copied().collect();
        let on_screen_new: Vec<_> = on_screen
            .into_iter()
            .filter(|id| !already.contains(id))
            .collect();
        let (added, msg) = if !on_screen_new.is_empty() {
            (on_screen_new, "GUI:SelectedAcrossScreen")
        } else {
            let map_new: Vec<_> = candidates
                .into_iter()
                .filter(|id| !already.contains(id))
                .collect();
            if !map_new.is_empty() {
                (map_new, "GUI:SelectedAcrossMap")
            } else if kept.is_empty() {
                (Vec::new(), "GUI:NothingSelected")
            } else {
                (Vec::new(), "GUI:SelectedAcrossMap")
            }
        };
        if added.is_empty() && msg == "GUI:NothingSelected" {
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        for id in added {
            if !kept.contains(&id) {
                kept.push(id);
            }
        }
        self.host_set_selection(self.current_player_id, kept);
        if !self.selected_objects.is_empty() {
            self.play_sound_effect(SoundType::Select);
        }
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Retail SELECT_ALL_AIRCRAFT (KEY_W) residual.
    pub(super) fn select_all_friendly_aircraft(&mut self) {
        self.select_all_units_by_type(true);
    }
    /// Retail SELECT_HERO (Ctrl+H) residual.
    /// C++ `CommandXlat.cpp:822-834` `iNeedAHero` + `:2801-2834` `MSG_META_SELECT_HERO`.
    pub(super) fn select_hero_units_hotkey(&mut self) {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        // Presentation-owned hero identity (no live GameLogic dual-scan).
        let _ = frame.alive_selectable_friendly_hero_ids(frame.local_team());
        let Some((target_id, look)) = select_hero_from_frame(frame, frame.local_team()) else {
            return;
        };
        self.host_set_selection(self.current_player_id, vec![target_id]);
        self.play_sound_effect(SoundType::Select);
        let _ = self.host_center_camera_and_request_focus(look);
    }

    /// Retail SELECT_MATCHING_UNITS (KEY_E) residual — type-select from current selection.
    /// C++ `InGameUI::selectUnitsMatchingCurrentSelection` (`InGameUI.cpp:4900`):
    /// screen pass first, then map; union every locally-controlled selected
    /// template; add (`MSG_CREATE_SELECTED_GROUP_NO_SOUND`, createNewGroup=false).
    pub(super) fn select_matching_units_hotkey(&mut self) {
        let current = self.ui_selected_ids(self.current_player_id);
        let plan = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            let team = frame.local_team();
            let mut map_matches: Vec<ObjectId> = Vec::new();
            let mut had_local_template = false;
            let mut first_selected_is_structure = false;
            for (index, id) in current.iter().enumerate() {
                let Some(obj) = frame.objects.iter().find(|object| object.id == *id) else {
                    continue;
                };
                if index == 0 {
                    first_selected_is_structure = obj.is_structure
                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                            obj,
                            crate::game_logic::KindOf::Structure,
                        )
                        || obj.object_type
                            == crate::presentation_frame::PresentationObjectType::Building;
                }
                if !frame.is_owned_by_local(obj) {
                    continue;
                }
                had_local_template = true;
                for match_id in frame.similar_unit_ids(*id, team) {
                    if !map_matches.contains(&match_id) {
                        map_matches.push(match_id);
                    }
                }
            }
            let (vw, vh) = self.tactical_viewport_size();
            let viewport = glam::Vec2::new(vw, vh);
            let screen_matches = frame.filter_ids_on_screen(
                &map_matches,
                self.view_matrix,
                self.projection_matrix,
                viewport,
            );
            let screen_new: Vec<ObjectId> = screen_matches
                .into_iter()
                .filter(|id| !current.contains(id))
                .collect();
            let map_new: Vec<ObjectId> = map_matches
                .into_iter()
                .filter(|id| !current.contains(id))
                .collect();
            matching_units_hotkey_plan(
                had_local_template,
                &screen_new,
                &map_new,
                first_selected_is_structure,
            )
        };
        if !plan.added.is_empty() {
            let mut selection = current;
            for id in plan.added {
                if !selection.contains(&id) {
                    selection.push(id);
                }
            }
            self.host_set_selection(self.current_player_id, selection);
        }
        if let Some(msg) = plan.message {
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
        }
    }
}

/// C++ `selectMatchingAcrossScreen` then `selectMatchingAcrossMap` HUD + add set.
struct MatchingUnitsHotkeyPlan {
    added: Vec<ObjectId>,
    message: Option<&'static str>,
}

fn matching_units_hotkey_plan(
    had_local_template: bool,
    screen_new: &[ObjectId],
    map_new: &[ObjectId],
    first_selected_is_structure: bool,
) -> MatchingUnitsHotkeyPlan {
    if !had_local_template {
        return MatchingUnitsHotkeyPlan {
            added: Vec::new(),
            message: Some("GUI:NothingSelected"),
        };
    }
    if !screen_new.is_empty() {
        return MatchingUnitsHotkeyPlan {
            added: screen_new.to_vec(),
            message: Some("GUI:SelectedAcrossScreen"),
        };
    }
    let message = if !map_new.is_empty() || !first_selected_is_structure {
        Some("GUI:SelectedAcrossMap")
    } else {
        None
    };
    MatchingUnitsHotkeyPlan {
        added: map_new.to_vec(),
        message,
    }
}

/// C++ `iNeedAHero` first `KINDOF_HERO`, then `getContainedBy()` + lookAt.
fn select_hero_from_frame(
    frame: &crate::presentation_frame::PresentationFrame,
    team: crate::game_logic::Team,
) -> Option<(ObjectId, glam::Vec3)> {
    let hero = frame.objects.iter().find(|object| {
        object.team == team
            && !object.destroyed
            && crate::presentation_frame::PresentationFrame::object_has_kind(
                object,
                crate::game_logic::KindOf::Hero,
            )
    })?;
    let target_id = hero.contained_by.unwrap_or(hero.id);
    let look = frame
        .objects
        .iter()
        .find(|object| object.id == target_id)
        .map(|object| object.position)
        .unwrap_or(hero.position);
    Some((target_id, look))
}

#[cfg(test)]
mod idle_worker_selection_tests {
    use super::*;

    fn target(worker: u32, selected: u32, x: f32) -> IdleWorkerSelectionTarget {
        IdleWorkerSelectionTarget {
            worker_id: ObjectId(worker),
            selection_id: ObjectId(selected),
            focus_position: glam::Vec3::new(x, 0.0, x + 10.0),
        }
    }

    #[test]
    fn control_bar_idle_worker_choice_matches_cpp_first_next_wrap_and_container_target() {
        // The second target models an idle worker inside selectable container 90.
        let targets = [target(10, 10, 10.0), target(20, 90, 20.0)];

        // C++ starts at the first idle worker for zero, multiple, or unrelated selection.
        for selected in [vec![], vec![ObjectId(77)], vec![ObjectId(10), ObjectId(20)]] {
            assert_eq!(
                select_next_idle_worker_target(&targets, &selected),
                Some(targets[0])
            );
        }
        // One selected idle worker advances and wraps through the idle-list order.
        assert_eq!(
            select_next_idle_worker_target(&targets, &[ObjectId(10)]),
            Some(targets[1])
        );
        assert_eq!(
            select_next_idle_worker_target(&targets, &[ObjectId(20)]),
            Some(targets[0])
        );
        // A contained worker's cycle identity remains the worker, while the
        // host selection/camera payload targets its containing object like C++.
        assert_eq!(targets[1].worker_id, ObjectId(20));
        assert_eq!(targets[1].selection_id, ObjectId(90));
        assert_eq!(targets[1].focus_position, glam::Vec3::new(20.0, 0.0, 30.0));
        assert!(select_next_idle_worker_target(&[], &[]).is_none());
    }

    #[test]
    fn key_e_matching_plan_is_screen_then_map_add() {
        // C++ InGameUI.cpp:4900-4916 selectUnitsMatchingCurrentSelection.
        let screen = [ObjectId(2)];
        let map = [ObjectId(2), ObjectId(3)];
        let plan = matching_units_hotkey_plan(true, &screen, &map, false);
        assert_eq!(plan.added, vec![ObjectId(2)]);
        assert_eq!(plan.message, Some("GUI:SelectedAcrossScreen"));
        let plan = matching_units_hotkey_plan(true, &[], &map, false);
        assert_eq!(plan.added, vec![ObjectId(2), ObjectId(3)]);
        assert_eq!(plan.message, Some("GUI:SelectedAcrossMap"));
        let plan = matching_units_hotkey_plan(false, &[], &[], false);
        assert!(plan.added.is_empty());
        assert_eq!(plan.message, Some("GUI:NothingSelected"));
    }

    #[test]
    fn context_pick_profile_matches_cpp_selection_info() {
        // C++ SelectionInfo.cpp:227-295.
        assert_eq!(
            host_context_pick_profile(true, None, false),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: false,
                include_force_attackable: true,
            }
        );
        assert_eq!(
            host_context_pick_profile(true, None, true),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: true,
                include_force_attackable: true,
            }
        );
        assert_eq!(
            host_context_pick_profile(false, None, true),
            HostContextPickProfile::default()
        );
        assert_eq!(
            host_context_pick_profile(true, Some(CMD_ALLOW_MINE_TARGET), true),
            HostContextPickProfile {
                include_mines: true,
                include_shrubbery: false,
                include_force_attackable: true,
            }
        );
        assert_eq!(
            host_context_pick_profile(true, Some(CMD_ALLOW_SHRUBBERY_TARGET), false),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: true,
                include_force_attackable: true,
            }
        );
        // Armed GUI command with no extra bits must not fall through to flame.
        assert_eq!(
            host_context_pick_profile(true, Some(0), true),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: false,
                include_force_attackable: true,
            }
        );
    }

    #[test]
    fn force_attack_pick_hits_forceattackable_only_fence() {
        // C++ KindOf.h: FORCEATTACKABLE is pickable via force-attack even if
        // not Selectable. Civ fences / cargo planes use this bit.
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;
        let mut logic = GameLogic::new();
        let mut fence = ThingTemplate::new("CivWoodenFence");
        fence.set_health(50.0);
        fence.add_kind_of(KindOf::ForceAttackable);
        logic.templates.insert("CivWoodenFence".into(), fence);
        let id = logic
            .create_object("CivWoodenFence", Team::USA, glam::Vec3::ZERO)
            .expect("fence");

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let stamped = frame.objects.iter().find(|o| o.id == id).expect("stamped");
        assert!(
            stamped.is_force_attackable,
            "presentation must freeze KINDOF_FORCEATTACKABLE"
        );
        assert!(!crate::unit_control::UnitControlSystem::presentation_is_selectable(stamped));
        assert!(!crate::unit_control::UnitControlSystem::presentation_is_attackable(stamped));
        let armed = host_context_pick_profile(true, None, false);
        assert_eq!(
            pick_widened_context_target(&frame, glam::Vec3::ZERO, Some(Team::USA), 20.0, armed),
            Some(id)
        );
        assert_eq!(
            pick_widened_context_target(
                &frame,
                glam::Vec3::ZERO,
                Some(Team::USA),
                20.0,
                HostContextPickProfile::default()
            ),
            None
        );
        let camera = glam::Vec3::new(0.0, 80.0, 80.0);
        assert_eq!(
            pick_widened_context_target_along_ray(
                &frame,
                camera,
                glam::Vec3::ZERO,
                Some(Team::USA),
                armed
            ),
            Some(id)
        );
    }
}
