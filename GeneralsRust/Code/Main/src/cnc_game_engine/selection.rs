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

/// Live-host analog of leftover `ContextPickProfile` mine/shrubbery bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HostContextPickProfile {
    include_mines: bool,
    include_shrubbery: bool,
}

/// C++ `getPickTypesForContext` / `getPickTypesForCurrentSelection`
/// (`SelectionInfo.cpp:227-295`). An armed GUI command owns the extra
/// pick bits; otherwise force-attack + a `DAMAGE_FLAME` weapon adds
/// shrubbery. Disarm no longer auto-picks mines.
pub(super) fn host_context_pick_profile(
    force_attack_mode: bool,
    armed_gui_command_options: Option<u32>,
    selection_has_flame: bool,
) -> HostContextPickProfile {
    let mut profile = HostContextPickProfile::default();
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

fn presentation_object_has_flame_weapon(
    o: &crate::presentation_frame::RenderableObject,
) -> bool {
    let Some(name) = crate::game_logic::primary_weapon_name_for_unit(&o.template_name) else {
        return false;
    };
    crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(name)
        == crate::game_logic::combat::DamageType::Flame
}

/// Nearest mine/shrubbery under the cursor when the pick profile widens.
pub(super) fn pick_widened_context_target(
    frame: &crate::presentation_frame::PresentationFrame,
    position: glam::Vec3,
    player_team: Option<crate::game_logic::Team>,
    base_selection_radius: f32,
    profile: HostContextPickProfile,
) -> Option<ObjectId> {
    if !profile.include_mines && !profile.include_shrubbery {
        return None;
    }


    let mut best: Option<(ObjectId, f32)> = None;
    for o in &frame.objects {
        if o.destroyed {
            continue;
        }
        let is_local = player_team.is_some() && frame.is_owned_by_local(o);
        if !is_local && o.fow_visibility.visibility_alpha < 0.95 {
            continue;
        }
        let extra = (profile.include_mines && presentation_is_mine_pick(o))
            || (profile.include_shrubbery && presentation_is_shrubbery_pick(o));
        if !extra {
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
            if ed < sd {
                Some(e)
            } else {
                Some(s)
            }
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
            all.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = all.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    all[i]
                })
                .unwrap_or(all[0])
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
            self.host_center_camera_on(pos);
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

        // C++ NEXT walks the prepended drawable list backwards; PREV walks forward.
        let next = if let Some(current) = self.selected_objects.first().copied() {
            workers
                .iter()
                .position(|(id, _)| *id == current)
                .map(|idx| {
                    let n = workers.len() as i32;
                    let step = if delta >= 0 { -1 } else { 1 };
                    let i = (idx as i32 + step).rem_euclid(n) as usize;
                    workers[i]
                })
                .unwrap_or(if delta >= 0 {
                    workers[workers.len() - 1]
                } else {
                    workers[0]
                })
        } else if delta >= 0 {
            workers[workers.len() - 1]
        } else {
            workers[0]
        };

        self.host_set_selection(self.current_player_id, vec![next.0]);
        self.host_center_camera_on(next.1);
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
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
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
            let msg = "No unfinished construction to resume";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        };
        if builders.is_empty() {
            let msg = "No dozer/worker available to resume";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = "Resuming construction";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = "Unfinished construction selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = "Damaged structure selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select all idle friendly combat units residual (Ctrl+I).

    /// Purchase next available GeneralsExperience science residual (Alt+G).
    pub(super) fn try_purchase_next_generals_science(&mut self) {
        // Wave 234: science points/team prefer presentation freeze for UI gate.
        let player_id = self.current_player_id;
        let spp = self.ui_local_science_purchase_points();
        if spp <= 0 {
            let msg = "No science purchase points";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // C++ ControlBar.cpp:143-485 populatePurchaseScience — open the purchase
        // screen. First-capable science comes from Science.ini residual graph,
        // not a hardcoded 5-name faction array.
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheControlBar::toggle_purchase_science();
        }
        let unlocked_vec: Vec<String> = self.presentation_or_boot_unlocked_sciences(player_id);
        let unlocked: std::collections::HashSet<String> = unlocked_vec.into_iter().collect();
        let Some(science_name) =
            crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::first_capable_purchase_science_residual(
                &unlocked,
                spp,
            )
        else {
            let msg = format!("No purchasable science (spp={spp})");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            return;
        };

        // Wave 584: host queue purchase-science residual.
        self.host_queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::PurchaseScience {
                science_name: science_name.clone(),
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        let msg = format!("Purchased {science_name}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
        self.play_sound_effect(SoundType::Command);
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
        let msg = "Idle military selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No repairing units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} repairing", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No idle military units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        let msg = format!("Selected {} idle military", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No harvesters found";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        let msg = format!("Selected {} harvesters", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No idle harvesters";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} idle harvesters", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
        let label = match tab {
            ConstructionTab::Buildings => "Buildings",
            ConstructionTab::Infantry => "Infantry",
            ConstructionTab::Vehicles => "Vehicles",
            ConstructionTab::Aircraft => "Aircraft",
            ConstructionTab::NavalUnits => "Naval",
            ConstructionTab::SuperWeapons => "Superweapons",
        };
        let msg = format!("Construction tab: {label}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No structures found";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} structures", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Adjust guard radius on selected guarding units residual (Alt+[ / ]).

    /// Clear movement path / waypoints on selection residual (Alt+Z).
    pub(super) fn clear_selected_path_waypoints(&mut self) {
        // Wave 225: selection via presentation-first ui_selected_ids; mutation via GameLogic API.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            if self.sticky_waypoint_mode {
                self.sticky_waypoint_mode = false;
                let msg = "Waypoint mode: OFF";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "Path cleared";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = "Damaged unit selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    pub(super) fn adjust_selected_guard_radius(&mut self, delta: f32) {
        // Wave 225: selection via presentation-first ui_selected_ids; mutation via GameLogic API.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }
        let mut any = false;
        let mut last_r = 0.0_f32;
        for id in selected {
            if let Some(r) = self.host_adjust_unit_guard_radius(id, delta) {
                last_r = r;
                any = true;
            }
        }

        if any {
            let msg = format!("Guard radius: {last_r:.0}");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No attacking units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} attacking", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No units to stop";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = format!("Stopped {} units", ids.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No moving units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} moving", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No occupied transports";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!(
            "Selected {} occupied transports",
            self.selected_objects.len()
        );
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Toggle attack-order line drawing residual (Ctrl+F4).
    pub(super) fn toggle_attack_lines_hotkey(&mut self) {
        self.show_attack_lines = !self.show_attack_lines;
        let msg = if self.show_attack_lines {
            "Attack lines: ON"
        } else {
            "Attack lines: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Toggle movement path line drawing residual (Ctrl+F3).
    pub(super) fn toggle_move_lines_hotkey(&mut self) {
        self.show_move_lines = !self.show_move_lines;
        let msg = if self.show_move_lines {
            "Move lines: ON"
        } else {
            "Move lines: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No garrisoned structures";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} garrisoned", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    pub(super) fn toggle_fps_counter_hotkey(&mut self) {
        self.show_fps = !self.show_fps;
        let msg = if self.show_fps {
            "FPS counter: ON"
        } else {
            "FPS counter: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No control groups";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let team = frame.local_team();
        let selection = frame.filter_alive_selectable_ids(&stored, team);
        if selection.is_empty() {
            let msg = format!("Control group {group_num} empty");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.last_control_group_select = Some((group_num, Instant::now()));
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Control group {group_num}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No stealthed units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} stealthed", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No veteran units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} veterans", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No docked aircraft";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} docked aircraft", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    pub(super) fn toggle_debug_info_hotkey(&mut self) {
        self.show_debug_info = !self.show_debug_info;
        let msg = if self.show_debug_info {
            "Debug overlay: ON"
        } else {
            "Debug overlay: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
        let msg = "Busy producer selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No patrolling units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} patrolling", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No gathering units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} gathering", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
        let msg = "Ready special power selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No guarding units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} guarding", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    pub(super) fn toggle_health_bars_hotkey(&mut self) {
        self.show_health_bars = !self.show_health_bars;
        let msg = if self.show_health_bars {
            "Health bars: ON"
        } else {
            "Health bars: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No combat units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} combat units", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
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
            let msg = "No units on screen";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} on screen", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Toggle camera follow on primary selection residual (Alt+F).

    /// Snap camera to centroid of current selection residual (Alt+Space).
    pub(super) fn center_camera_on_selection(&mut self) {
        // Wave 234: selection prefers engine/presentation freeze.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            let msg = "Nothing selected";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Presentation-only poses for InGame camera center.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let Some(center) = frame.centroid_of_ids(&selected) else {
            return;
        };
        self.host_center_camera_on(center);
        let msg = "Centered on selection";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
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
            let msg = "No constructing workers";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, ids);
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} constructing", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    pub(super) fn toggle_camera_follow_selection(&mut self) {
        // Wave 548: presentation freeze owns follow-active residual when installed
        // (`camera_follow_position`; no live `camera_follow_object_id` dual-read).
        // Command authority still writes `set_camera_follow_object` for host follow state.
        // Wave 583: follow-active via presentation_or_boot helper.
        let follow_active = self.presentation_or_boot_camera_follow_active();
        if follow_active {
            self.host_set_camera_follow_object(None);
            let msg = "Camera follow off";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let id = self.ui_selection_seed_id();
        let Some(id) = id else {
            let msg = "Select a unit to follow";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        };
        self.host_set_camera_follow_object(Some(id));
        let msg = "Camera follow on";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Retail SELECT_ALL (KEY_Q) / Ctrl+A residual.
    /// C++ `InGameUI::selectAllUnitsByType`: screen pass then map fallback
    /// (`InGameUI.cpp:4877`). CommandXlat excludes DOZER/HARVESTER/IGNORES_SELECT_ALL
    /// and 1.03-incompatible current selection.
    pub(super) fn select_all_friendly_units(&mut self) {
        self.select_all_units_by_type(false);
    }

    fn select_all_units_by_type(&mut self, aircraft_only: bool) {
        let (team, incompatible, candidates, on_screen) = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            let team = frame.local_team();
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
            let candidates = frame.alive_select_all_unit_ids(team, aircraft_only);
            let window_size = self.window.inner_size();
            let viewport = glam::Vec2::new(window_size.width as f32, window_size.height as f32);
            let on_screen = frame.filter_ids_on_screen(
                &candidates,
                self.view_matrix,
                self.projection_matrix,
                viewport,
            );
            (team, incompatible, candidates, on_screen)
        };
        let _ = team;
        if incompatible {
            self.host_set_selection(self.current_player_id, Vec::new());
        }
        let (selection, msg) = if !on_screen.is_empty() {
            (on_screen, "GUI:SelectedAcrossScreen")
        } else if !candidates.is_empty() {
            (candidates, "GUI:SelectedAcrossMap")
        } else {
            (Vec::new(), "GUI:NothingSelected")
        };
        if selection.is_empty() && msg == "GUI:NothingSelected" {
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.host_set_selection(self.current_player_id, selection);
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
            let window_size = self.window.inner_size();
            let viewport = glam::Vec2::new(window_size.width as f32, window_size.height as f32);
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
            HostContextPickProfile::default()
        );
        assert_eq!(
            host_context_pick_profile(true, None, true),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: true,
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
            }
        );
        assert_eq!(
            host_context_pick_profile(true, Some(CMD_ALLOW_SHRUBBERY_TARGET), false),
            HostContextPickProfile {
                include_mines: false,
                include_shrubbery: true,
            }
        );
        // Armed GUI command with no extra bits must not fall through to flame.
        assert_eq!(
            host_context_pick_profile(true, Some(0), true),
            HostContextPickProfile::default()
        );
    }


}
