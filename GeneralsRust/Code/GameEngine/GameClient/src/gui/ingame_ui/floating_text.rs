// Floating combat/resource text.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.
//
// C++: addFloatingText / updateFloatingText / drawFloatingText
// (InGameUI.cpp:4955-5115).

impl InGameUI {
    pub fn add_floating_text(&mut self, text: String, position: Coord3D, color: (u8, u8, u8)) {
        // C++: gated on TheGameLogic->getDrawIconUI()
        if !TheGameLogic::get_draw_icon_ui() {
            return;
        }
        if self.floating_texts.len() >= MAX_FLOATING_TEXT {
            self.floating_texts.remove(0);
        }
        let timeout = if self.floating_text_timeout_frames == 0 {
            DEFAULT_FLOATING_TEXT_TIMEOUT
        } else {
            self.floating_text_timeout_frames
        };
        self.floating_texts.push(FloatingTextData {
            text,
            position,
            color,
            creation_frame: self.current_frame,
            timeout,
            move_up_speed: self.floating_text_move_up_speed,
            frame_count: 0,
            frame_timeout: self.current_frame + timeout,
            alpha: 255,
        });
    }

    pub fn clear_floating_texts(&mut self) {
        self.floating_texts.clear();
    }

    /// Wave 1060: replace floating texts from presentation freeze residual.
    pub fn replace_floating_texts_from_presentation(
        &mut self,
        entries: &[(String, Coord3D, (u8, u8, u8), u32, u32)],
    ) {
        self.floating_texts.clear();
        for (text, position, color, creation_frame, timeout) in entries {
            if self.floating_texts.len() >= MAX_FLOATING_TEXT {
                break;
            }
            let timeout = if *timeout == 0 {
                self.floating_text_timeout_frames.max(DEFAULT_FLOATING_TEXT_TIMEOUT)
            } else {
                *timeout
            };
            self.floating_texts.push(FloatingTextData {
                text: text.clone(),
                position: *position,
                color: *color,
                creation_frame: *creation_frame,
                timeout,
                move_up_speed: self.floating_text_move_up_speed,
                frame_count: 0,
                frame_timeout: creation_frame.saturating_add(timeout),
                alpha: 255,
            });
        }
    }

    /// C++ InGameUI::updateFloatingText (InGameUI.cpp:5030-5077).
    pub fn update_floating_texts(&mut self) {
        if self.last_ui_logic_frame == self.current_frame {
            return;
        }
        let current_frame = self.current_frame;
        let vanish = self.floating_text_vanish_rate;
        self.floating_texts.retain_mut(|ft| {
            ft.frame_count = ft.frame_count.saturating_add(1);
            if current_frame > ft.frame_timeout {
                let amount = ((current_frame - ft.frame_timeout) as f32 * vanish) as i32;
                let new_a = (ft.alpha as i32 - amount).max(0) as u8;
                ft.alpha = new_a;
                new_a > 0
            } else {
                true
            }
        });
    }

    /// Screen-Y rise used by draw. C++: pos.y -= frameCount * moveUpSpeed
    pub fn floating_text_screen_offset_y(frame_count: u32, move_up_speed: f32) -> f32 {
        frame_count as f32 * move_up_speed
    }

    pub fn floating_text_visible_through_shroud(status: ObjectShroudStatus) -> bool {
        status == ObjectShroudStatus::Clear
    }
}
