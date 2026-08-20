// Radius cursor targeting overlay.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.
//
// C++: InGameUI::setRadiusCursor / handleRadiusCursor (InGameUI.cpp:1172-1310).

use crate::radius_decal::RadiusDecalTemplate;
use game_engine::common::ini::get_global_data;

use gamelogic::ai::guard::AIGuardMachine;
use gamelogic::weapon::WeaponSlotType;

const DEFAULT_RADIUS_DECAL_TEXTURE: &str = "SCCAttackDamageArea";

impl InGameUI {
    fn default_radius_cursor_templates() -> Vec<RadiusDecalTemplate> {
        Self::radius_cursor_templates_from_ini()
    }

    /// C++ `InGameUI::setRadiusCursor` (InGameUI.cpp:1172-1270).
    pub fn set_radius_cursor(
        &mut self,
        cursor_type: RadiusCursorType,
        position: Coord3D,
        radius: f32,
    ) {
        if cursor_type == self.radius_cursor.cursor_type && self.radius_cursor.active {
            self.radius_cursor.position = position;
            if radius > 0.0 {
                self.radius_cursor.radius = radius;
            }
            self.handle_radius_cursor();
            return;
        }
        if cursor_type == RadiusCursorType::None {
            self.clear_radius_cursor();
            return;
        }

        self.clear_radius_cursor();

        let resolved_radius = self.resolve_radius_cursor_radius(cursor_type, radius);
        if resolved_radius <= 0.0 {
            return;
        }

        let controller = self
            .first_selected_object_for_radius_cursor()
            .and_then(|id| OBJECT_REGISTRY.get_object(id))
            .and_then(|obj| obj.read().ok().and_then(|guard| guard.get_controlling_player()))
            .or_else(|| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
            });

        let Some(controller) = controller else {
            return;
        };

        if self.radius_cursor_templates.is_empty() {
            self.radius_cursor_templates = Self::default_radius_cursor_templates();
        }
        let index = cursor_type as u32 as usize;
        if index >= self.radius_cursor_templates.len() {
            return;
        }
        if !self.radius_cursor_templates[index].valid() {
            self.radius_cursor_templates[index].set_texture(DEFAULT_RADIUS_DECAL_TEXTURE);
        }

        let owner = controller;
        self.radius_cursor_templates[index].create_radius_decal(
            &Self::radius_decal_pos(position),
            resolved_radius,
            Some(owner),
            &mut self.cur_radius_decal,
        );

        self.radius_cursor.cursor_type = cursor_type;
        self.radius_cursor.active = !self.cur_radius_decal.is_empty();
        self.radius_cursor.position = position;
        self.radius_cursor.radius = resolved_radius;
        // C++ setRadiusCursor does not write m_duringDoubleClickAttackMoveGuardHintStashedPosition.
        self.handle_radius_cursor();
    }

    pub fn clear_radius_cursor(&mut self) {
        self.cur_radius_decal.clear();
        self.radius_cursor.cursor_type = RadiusCursorType::None;
        self.radius_cursor.active = false;
        self.radius_cursor.radius = 0.0;
    }

    pub fn is_radius_cursor_active(&self) -> bool {
        self.radius_cursor.active && !self.cur_radius_decal.is_empty()
    }

    pub fn get_radius_cursor_type(&self) -> RadiusCursorType {
        self.radius_cursor.cursor_type
    }

    pub fn update_radius_cursor(&mut self, mouse_pos: Coord3D) {
        if !self.radius_cursor.active {
            return;
        }
        self.radius_cursor.position = mouse_pos;
        self.handle_radius_cursor();
    }

    /// C++ `InGameUI::handleRadiusCursor` (InGameUI.cpp:1275-1310).
    pub fn handle_radius_cursor(&mut self) {
        if self.cur_radius_decal.is_empty() {
            return;
        }

        let (mx, my) = with_mouse(|mouse| mouse.state().position());
        let screen = IPoint2::new(mx as i32, my as i32);
        let pos = Self::radius_cursor_world_from_mouse(screen)
            .unwrap_or(self.radius_cursor.position);

        let live_timer = TheInGameUI::double_click_attack_move_guard_timer();
        let (guard_timer, stash) = if self.double_click_attack_move_guard_timer > 0 {
            (
                self.double_click_attack_move_guard_timer,
                self.guard_hint_stashed_position,
            )
        } else {
            let (x, y, z) = TheInGameUI::guard_hint_stashed_position();
            (live_timer, Coord3D::new(x, y, z))
        };
        let double_click_guard = get_global_data()
            .map(|data| data.read().double_click_attack_move)
            .unwrap_or(false)
            && guard_timer > 0;

        if double_click_guard {
            self.cur_radius_decal
                .set_opacity(guard_timer as f32 * 0.1);
            self.cur_radius_decal
                .set_position(&Self::radius_decal_pos(stash));

        } else {
            self.cur_radius_decal
                .set_position(&Self::radius_decal_pos(pos));
            self.cur_radius_decal.update();
        }
        self.radius_cursor.position = pos;
    }

    fn radius_cursor_world_from_mouse(screen: IPoint2) -> Option<Coord3D> {
        if let Some(radar_world) = Self::radar_screen_to_world(screen.x, screen.y) {
            return Some(radar_world);
        }
        with_tactical_view_ref(|view| {
            view.screen_to_terrain(&screen)
                .ok()
                .map(|point| Coord3D::new(point.x, point.y, point.z))
        })
    }

    fn radar_screen_to_world(mx: i32, my: i32) -> Option<Coord3D> {
        radar_screen_pixel_to_world(mx, my)
    }

    fn first_selected_object_for_radius_cursor(&self) -> Option<u32> {
        if let Some(pending) = TheInGameUI::get_pending_special_power() {
            if self
                .gui_command
                .as_deref()
                .is_some_and(|cmd| cmd.to_ascii_uppercase().contains("SHORTCUT"))
            {
                if let Some(id) = Self::most_ready_shortcut_object(pending.power_id) {
                    return Some(id);
                }
            }
        }
        self.get_selection().into_iter().next()
    }

    fn most_ready_shortcut_object(power_id: u32) -> Option<u32> {
        let template_name = get_special_power_store().and_then(|store| {
            store
                .find_special_power_template_by_id(power_id)
                .map(|t| t.get_name().to_string())
        })?;
        let mut best: Option<(u32, u32)> = None;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_locally_controlled() {
                continue;
            }
            let Some(ready_frame) =
                guard.with_special_power_module_interface_by_name(&template_name, |sp| {
                    sp.get_ready_frame()
                })
            else {
                continue;
            };
            match best {
                None => best = Some((guard.get_id(), ready_frame)),
                Some((_, best_frame)) if ready_frame < best_frame => {
                    best = Some((guard.get_id(), ready_frame));
                }
                _ => {}
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn resolve_radius_cursor_radius(&self, cursor_type: RadiusCursorType, requested: f32) -> f32 {
        if let Some(obj_id) = self.first_selected_object_for_radius_cursor() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(guard) = obj.read() {
                    let slot = WeaponSlotType::Primary;
                    match cursor_type {
                        RadiusCursorType::AttackDamageArea => {
                            if let Some(weapon) = guard.get_weapon_in_weapon_slot(slot) {
                                let radius = weapon.get_primary_damage_radius(obj_id);
                                if radius > 0.0 {
                                    return radius;
                                }
                            }
                        }
                        RadiusCursorType::AttackScatterArea => {
                            if let Some(weapon) = guard.get_weapon_in_weapon_slot(slot) {
                                let radius = weapon.get_scatter_radius()
                                    + weapon.get_scatter_target_scalar();
                                if radius > 0.0 {
                                    return radius;
                                }
                            }
                        }
                        RadiusCursorType::AttackContinueArea | RadiusCursorType::ClearMines => {
                            if let Some(weapon) = guard.get_weapon_in_weapon_slot(slot) {
                                let radius = weapon.get_continue_attack_range();
                                if radius > 0.0 {
                                    return radius;
                                }
                            }
                        }
                        RadiusCursorType::GuardArea => {
                            let radius = AIGuardMachine::get_std_guard_range(obj_id);
                            if radius > 0.0 {
                                return radius;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(pending) = TheInGameUI::get_pending_special_power() {
            if let Some(store) = get_special_power_store() {
                if let Some(template) = store.find_special_power_template_by_id(pending.power_id) {
                    let radius = template.get_radius_cursor_radius();
                    if radius > 0.0 {
                        return radius;
                    }
                }
            }
        }

        requested
    }

    fn radius_decal_pos(pos: Coord3D) -> game_engine::common::system::Coord3D {
        game_engine::common::system::Coord3D::new(pos.x, pos.y, pos.z)
    }
}
