// World-space 2D animations.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn add_world_animation(
        &mut self,
        animation_name: &str,
        pos: Coord3D,
        options: WorldAnimationOptions,
        duration_seconds: f32,
        z_rise_per_second: f32,
    ) {
        if duration_seconds <= 0.0 || animation_name.is_empty() {
            return;
        }

        let Some(collection) = get_anim2d_collection() else {
            return;
        };
        let collection_guard = collection.read();
        let template = collection_guard.find_template(&AsciiString::from(animation_name));
        let Some(template) = template else {
            return;
        };
        drop(collection_guard);

        let anim = crate::system::Anim2D::new(template, None);

        let expire_frame = self.current_frame + (duration_seconds * 30.0) as u32;
        self.world_animations.push(WorldAnimationData {
            anim,
            world_pos: pos,
            expire_frame,
            options,
            z_rise_per_second,
        });
    }

    pub fn clear_world_animations(&mut self) {
        self.world_animations.clear();
    }

    pub fn update_and_draw_world_animations(&mut self) {
        const FRAMES_BEFORE_EXPIRE_TO_FADE: u32 = 30;

        let current_frame = self.current_frame;
        let paused = TheGameLogic::is_game_paused();

        let local_player_index = gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|g| g.get_player_index() as u32));

        let mut i = 0;
        while i < self.world_animations.len() {
            let expired = if !paused {
                current_frame >= self.world_animations[i].expire_frame
                    || (self.world_animations[i]
                        .options
                        .contains(WorldAnimationOptions::PLAY_ONCE_AND_DESTROY)
                        && self.world_animations[i]
                            .anim
                            .lock()
                            .get_status()
                            .contains(crate::system::Anim2DStatus::COMPLETE))
            } else {
                current_frame >= self.world_animations[i].expire_frame
            };

            if expired {
                self.world_animations.remove(i);
                continue;
            }

            if !paused && self.world_animations[i].z_rise_per_second != 0.0 {
                self.world_animations[i].world_pos.z +=
                    self.world_animations[i].z_rise_per_second / 30.0;
            }

            let shrouded = local_player_index
                .map(|player_idx| {
                    get_shroud_manager()
                        .lock()
                        .ok()
                        .map(|shroud| {
                            shroud.get_shroud_state(player_idx, &self.world_animations[i].world_pos)
                                != ShroudState::Visible
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if shrouded {
                i += 1;
                continue;
            }

            if self.world_animations[i]
                .options
                .contains(WorldAnimationOptions::FADE_ON_EXPIRE)
            {
                let frames_till_expire = self.world_animations[i]
                    .expire_frame
                    .saturating_sub(current_frame);
                if frames_till_expire < FRAMES_BEFORE_EXPIRE_TO_FADE {
                    let alpha = frames_till_expire as f32 / FRAMES_BEFORE_EXPIRE_TO_FADE as f32;
                    self.world_animations[i].anim.lock().set_alpha(alpha);
                }
            }

            let screen = self.world_to_screen(&self.world_animations[i].world_pos);
            if let Some(screen) = screen {
                let mut anim_guard = self.world_animations[i].anim.lock();
                let width = anim_guard.get_current_frame_width() as f32;
                let height = anim_guard.get_current_frame_height() as f32;

                let zoom_scale = with_tactical_view_ref(|view| {
                    let max_zoom = view.max_zoom();
                    let zoom = view.zoom();
                    if zoom > 0.0 {
                        max_zoom / zoom
                    } else {
                        1.0
                    }
                });

                let scaled_width = (width * zoom_scale) as i32;
                let scaled_height = (height * zoom_scale) as i32;

                let draw_x = (screen.x - scaled_width as f32 / 2.0) as i32;
                let draw_y = (screen.y - scaled_height as f32 / 2.0) as i32;

                anim_guard.draw_sized(draw_x, draw_y, scaled_width, scaled_height);
            }

            i += 1;
        }
    }

    // ── Lifecycle methods ──────────────────────────────────────────────
    // C++: InGameUI.cpp:1571 (preDraw)
    // C++: InGameUI.cpp:3426 (postDraw)

}
