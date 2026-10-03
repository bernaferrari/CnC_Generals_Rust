//! Combat-drop ropes, rappellers and lifecycle.

use super::{
    ChinookAIUpdate, ChinookCombatDropState, ChinookFlightStatus, INVALID_DRAWABLE_ID, RopeInfo,
    chinook_dump_owner_crate_visuals, dual_world_registry_unavailable,
};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND, Real, UnsignedInt};
use crate::helpers::{
    TheGameClient, TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
};
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::Object;
use crate::object::draw::draw_module::RGBColor;
use crate::object::drawable::{Drawable, DrawableArcExt};
use game_engine::common::global_data;
use std::sync::{Arc, RwLock};

impl ChinookAIUpdate {
    fn get_potential_rappeller(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 349: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner = TheGameLogic::find_object_by_id(self.object_id)?;
        let owner_guard = owner.read().ok()?;
        let contain = owner_guard.get_contain()?;
        for object_id in contain.get_contained_objects() {
            let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let is_rappeller = if let Ok(obj_guard) = obj.read() {
                obj_guard.is_kind_of(KindOf::CanRappel)
            } else {
                false
            };
            if is_rappeller {
                return Some(obj);
            }
        }
        None
    }

    fn init_rope_draw_params(
        drawable: &Arc<RwLock<Drawable>>,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    ) {
        drawable.init_rope_draw_params(length, width, color, wobble_len, wobble_amp, wobble_rate);
    }

    fn set_rope_cur_len(drawable: &Arc<RwLock<Drawable>>, length: Real) {
        drawable.set_rope_cur_len(length);
    }

    fn set_rope_speed(
        drawable: &Arc<RwLock<Drawable>>,
        cur_speed: Real,
        max_speed: Real,
        accel: Real,
    ) {
        drawable.set_rope_speed(cur_speed, max_speed, accel);
    }

    pub(super) fn start_combat_drop(&mut self) -> bool {
        // Wave 349: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return false;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return false;
        };
        let Some(drawable) = owner_guard.get_drawable() else {
            return false;
        };
        let Ok(draw_guard) = drawable.read() else {
            return false;
        };

        owner_guard.set_disabled(crate::common::DisabledType::Held);
        // C++ ChinookCombatDropState::onEnter: while (ai->loseOneBox()).
        while self.base.lose_one_box() {}
        let now = TheGameLogic::get_frame();
        let rope_template = TheThingFactory::find_template(self.data.rope_name.as_str());
        let mut rope_positions = draw_guard.get_pristine_bone_positions("RopeStart", 1, 32);
        let mut drop_transforms = draw_guard.get_pristine_bone_transforms("RopeEnd", 1, 32);
        drop(draw_guard);
        chinook_dump_owner_crate_visuals(&owner_guard, self.base.get_max_boxes());

        let mut num_ropes = self.data.num_ropes as usize;
        if num_ropes > rope_positions.len() {
            num_ropes = rope_positions.len();
        }
        if num_ropes > drop_transforms.len() {
            num_ropes = drop_transforms.len();
        }
        if num_ropes == 0 {
            return false;
        }

        rope_positions.truncate(num_ropes);
        drop_transforms.truncate(num_ropes);

        let mut ropes = Vec::with_capacity(num_ropes);
        for i in 0..num_ropes {
            let drop_start_mtx =
                owner_guard.convert_bone_pos_to_world_pos(None, Some(&drop_transforms[i]));

            let (rope_drawable_id, rope_drawable) = if let (Some(template), Some(client)) =
                (rope_template.as_ref(), TheGameClient::get())
            {
                let id = client.create_drawable(template.as_ref());
                let drawable_arc = client.get_drawable_arc(id);
                (id, drawable_arc)
            } else {
                (INVALID_DRAWABLE_ID, None)
            };

            if let Some(rope_drawable) = rope_drawable.as_ref() {
                let rope_world_mtx =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&rope_positions[i]), None);
                if let Ok(mut rope_guard) = rope_drawable.write() {
                    rope_guard.set_transform(rope_world_mtx);
                }
            }

            let mut rope_len_max = 0.0;
            if let Some(terrain) = TheTerrainLogic::get() {
                let rope_world_mtx =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&rope_positions[i]), None);
                let (_, _, translation) = rope_world_mtx.to_scale_rotation_translation();
                let rope_pos = Coord3D::new(translation.x, translation.y, translation.z);
                let layer = terrain.get_highest_layer_for_destination(&rope_pos);
                let ground = terrain.get_layer_height(rope_pos.x, rope_pos.y, layer);
                rope_len_max = rope_pos.z - ground - self.data.rope_final_height;
            }

            if let Some(rope_drawable) = rope_drawable.as_ref() {
                Self::init_rope_draw_params(
                    rope_drawable,
                    rope_len_max,
                    self.data.rope_width,
                    self.data.rope_color,
                    self.data.rope_wobble_len,
                    self.data.rope_wobble_amp,
                    self.data.rope_wobble_rate,
                );
            }

            let next_delay = get_game_logic_random_value(
                self.data.per_rope_delay_min as i32,
                self.data.per_rope_delay_max as i32,
            ) as UnsignedInt;

            ropes.push(RopeInfo {
                rope_drawable,
                rope_drawable_id,
                drop_start_mtx,
                rope_speed: 0.0,
                rope_len: 1.0,
                rope_len_max,
                next_drop_time: now + next_delay - self.data.per_rope_delay_min,
                rappeller_ids: Vec::new(),
            });
        }

        self.combat_drop_state = Some(ChinookCombatDropState { ropes });
        self.combat_drop_started = true;
        true
    }

    pub(super) fn update_combat_drop(&mut self) -> bool {
        // Wave 349: empty dual-world → true (drop residual cleared).
        if dual_world_registry_unavailable() {
            return true;
        }

        let Some(mut state) = self.combat_drop_state.take() else {
            return true;
        };
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        let Some(_contain) = owner_guard.get_contain() else {
            return true;
        };

        // remove done rappellers
        for rope in &mut state.ropes {
            rope.rappeller_ids.retain(|id| {
                let _ =
                    crate::object::registry::OBJECT_REGISTRY.with_object(*id, |rappeller_guard| {
                        return !rappeller_guard.is_effectively_dead()
                            && rappeller_guard.is_above_terrain();
                    });
                false
            });
        }

        let now = TheGameLogic::get_frame();
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);

        let mut ropes_in_use = 0;
        for rope in &mut state.ropes {
            if rope.rope_len < rope.rope_len_max {
                rope.rope_speed += gravity;
                if rope.rope_speed > self.data.rope_drop_speed {
                    rope.rope_speed = self.data.rope_drop_speed;
                }
                rope.rope_len += rope.rope_speed;
                if let Some(rope_drawable) = rope.rope_drawable.as_ref() {
                    Self::set_rope_cur_len(rope_drawable, rope.rope_len);
                }
                if self.data.wait_for_ropes_to_drop {
                    rope.next_drop_time = rope.next_drop_time.saturating_add(1);
                    continue;
                }
            }

            if now >= rope.next_drop_time {
                if let Some(rappeller) = self.get_potential_rappeller() {
                    let prepared = rappeller.read().ok().map(|rappeller_guard| {
                        let exit_interface = owner_guard.get_object_exit_interface();
                        let exit_door = exit_interface
                            .as_ref()
                            .and_then(|exit| {
                                exit.lock().ok().map(|mut guard| {
                                    guard.reserve_door_for_exit(
                                        Some(&*owner_guard),
                                        Some(&*rappeller_guard),
                                    )
                                })
                            })
                            .unwrap_or(crate::modules::DOOR_NONE_AVAILABLE);
                        (exit_interface, exit_door, rappeller_guard.get_id())
                    });
                    if let Some((exit_interface, exit_door, rappeller_id)) = prepared {
                        if exit_door != crate::modules::DOOR_NONE_AVAILABLE {
                            if let Some(exit) = exit_interface {
                                let _ = exit.lock().ok().map(|mut guard| {
                                    guard.exit_object_via_door(rappeller_id, exit_door)
                                });
                            }
                        }
                    }

                    if let Ok(mut rappeller_guard) = rappeller.write() {
                        rappeller_guard.set_transform_matrix(&rope.drop_start_mtx);
                    }

                    if let Ok(rappeller_guard) = rappeller.read() {
                        if let Some(ai) = rappeller_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard.set_desired_speed(self.data.rappel_speed);
                            }
                            let mut params = AiCommandParams::new(
                                AiCommandType::RappelInto,
                                CommandSourceType::FromAi,
                            );
                            params.obj = self.combat_drop_target;
                            params.pos = self.combat_drop_pos;
                            let _ = ai.execute_command(&params);
                        }
                    }

                    if let Ok(rappeller_guard) = rappeller.read() {
                        rope.rappeller_ids.push(rappeller_guard.get_id());
                    }

                    let next_delay = get_game_logic_random_value(
                        self.data.per_rope_delay_min as i32,
                        self.data.per_rope_delay_max as i32,
                    )
                    .max(self.data.per_rope_delay_min as i32)
                        as UnsignedInt;
                    rope.next_drop_time = now + next_delay;
                }
            }

            if !rope.rappeller_ids.is_empty() {
                ropes_in_use += 1;
            }
        }

        let done = ropes_in_use == 0 && self.get_potential_rappeller().is_none();
        if !done {
            self.combat_drop_state = Some(state);
        }
        done
    }

    pub(super) fn finish_combat_drop(&mut self, owner_dead: bool) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            self.combat_drop_state = None;
            self.combat_drop_started = false;
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            self.combat_drop_state = None;
            self.combat_drop_started = false;
            return;
        };

        owner_guard.clear_disabled(crate::common::DisabledType::Held);
        self.flight_status = ChinookFlightStatus::Flying;

        if owner_dead {
            if let Some(state) = self.combat_drop_state.as_ref() {
                for rope in &state.ropes {
                    for rappeller_id in &rope.rappeller_ids {
                        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(
                            *rappeller_id,
                            |rappeller_guard| {
                                if let Some(ai) = rappeller_guard.get_ai_update_interface() {
                                    ai.ai_idle(CommandSourceType::FromAi);
                                }
                            },
                        );
                    }
                }
            }
        }

        let now = TheGameLogic::get_frame();
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);
        if let Some(state) = self.combat_drop_state.take() {
            for rope in state.ropes {
                if let Some(rope_drawable) = rope.rope_drawable.as_ref() {
                    let initial_speed = gravity * 30.0;
                    Self::set_rope_speed(
                        rope_drawable,
                        initial_speed,
                        self.data.rope_drop_speed,
                        gravity,
                    );
                }
                if rope.rope_drawable_id != INVALID_DRAWABLE_ID {
                    if let Some(client) = TheGameClient::get() {
                        let expiration = LOGICFRAMES_PER_SECOND * 5;
                        client
                            .set_drawable_expiration_date(rope.rope_drawable_id, now + expiration);
                    }
                }
            }
        }
        self.combat_drop_started = false;
    }
}

impl ChinookAIUpdate {
    pub fn is_doing_combat_drop(&self) -> bool {
        self.flight_status == ChinookFlightStatus::DoingCombatDrop
            || self.combat_drop_started
            || self.combat_drop_state.is_some()
    }
}
