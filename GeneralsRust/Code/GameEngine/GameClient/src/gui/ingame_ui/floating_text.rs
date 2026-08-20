// Floating combat/resource text.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.
//
// C++: addFloatingText / updateFloatingText / drawFloatingText
// (InGameUI.cpp:4955-5115).

impl InGameUI {
    /// C++ `REAL_TO_INT((curr - timeout) * m_floatingTextMoveVanishRate)`
    /// (InGameUI.cpp:5057). Positive values truncate toward zero.
    pub fn floating_text_fade_amount(frames_past_timeout: u32, vanish_rate: f32) -> i32 {
        (frames_past_timeout as f32 * vanish_rate) as i32
    }

    /// Replay C++ per-frame vanish from `spawn_alpha` so a presentation
    /// restamp can drop texts that would already have been erased.
    /// C++ InGameUI.cpp:5053-5068.
    pub fn floating_text_alpha_at_frame(
        spawn_alpha: u8,
        current_frame: u32,
        frame_timeout: u32,
        vanish_rate: f32,
    ) -> u8 {
        if current_frame <= frame_timeout {
            return spawn_alpha;
        }
        let mut alpha = spawn_alpha as i32;
        let mut past = 1u32;
        let mut frame = frame_timeout.saturating_add(1);
        while frame <= current_frame {
            alpha -= Self::floating_text_fade_amount(past, vanish_rate);
            if alpha <= 0 {
                return 0;
            }
            past = past.saturating_add(1);
            frame = frame.saturating_add(1);
        }
        alpha as u8
    }

    pub fn add_floating_text(&mut self, text: String, position: Coord3D, color: (u8, u8, u8)) {
        // C++: gated on TheGameLogic->getDrawIconUI() (InGameUI.cpp:4957).
        if !TheGameLogic::get_draw_icon_ui() {
            return;
        }
        let timeout = if self.floating_text_timeout_frames == 0 {
            DEFAULT_FLOATING_TEXT_TIMEOUT
        } else {
            self.floating_text_timeout_frames
        };
        // C++ `m_floatingTextList.push_front` (InGameUI.cpp:4974).
        if self.floating_texts.len() >= MAX_FLOATING_TEXT {
            self.floating_texts.pop();
        }
        self.floating_texts.insert(
            0,
            FloatingTextData {
                text,
                position,
                color,
                creation_frame: self.current_frame,
                timeout,
                move_up_speed: self.floating_text_move_up_speed,
                frame_count: 0,
                frame_timeout: self.current_frame + timeout,
                alpha: 255,
            },
        );
    }

    pub fn clear_floating_texts(&mut self) {
        self.floating_texts.clear();
    }

    /// Wave 1060: replace floating texts from presentation freeze residual.
    /// Reconstruct rise/fade from spawn so restamps cannot revive expired text.
    pub fn replace_floating_texts_from_presentation(
        &mut self,
        entries: &[(String, Coord3D, (u8, u8, u8), u32, u32)],
    ) {
        self.floating_texts.clear();
        // C++ never inserts when DrawIconUI is off (InGameUI.cpp:4957).
        if !TheGameLogic::get_draw_icon_ui() {
            return;
        }
        let current = self.current_frame;
        let vanish = self.floating_text_vanish_rate;
        let default_timeout = self
            .floating_text_timeout_frames
            .max(DEFAULT_FLOATING_TEXT_TIMEOUT);
        for (text, position, color, creation_frame, timeout) in entries {
            if self.floating_texts.len() >= MAX_FLOATING_TEXT {
                break;
            }
            let timeout = if *timeout == 0 {
                default_timeout
            } else {
                *timeout
            };
            let frame_timeout = creation_frame.saturating_add(timeout);
            let alpha = Self::floating_text_alpha_at_frame(255, current, frame_timeout, vanish);
            if alpha == 0 {
                continue;
            }
            // Newest first, matching C++ push_front of each add.
            self.floating_texts.insert(
                0,
                FloatingTextData {
                    text: text.clone(),
                    position: *position,
                    color: *color,
                    creation_frame: *creation_frame,
                    timeout,
                    move_up_speed: self.floating_text_move_up_speed,
                    frame_count: current.saturating_sub(*creation_frame),
                    frame_timeout,
                    alpha,
                },
            );
        }
    }

    /// C++ InGameUI::updateFloatingText (InGameUI.cpp:5030-5077).
    pub fn update_floating_texts(&mut self) {
        // C++ static lastLogicFrameUpdate: one rise/fade step per logic frame.
        if self.last_ui_logic_frame == self.current_frame {
            return;
        }
        self.last_ui_logic_frame = self.current_frame;
        let current_frame = self.current_frame;
        let vanish = self.floating_text_vanish_rate;
        self.floating_texts.retain_mut(|ft| {
            ft.frame_count = ft.frame_count.saturating_add(1);
            if current_frame > ft.frame_timeout {
                let amount =
                    Self::floating_text_fade_amount(current_frame - ft.frame_timeout, vanish);
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

    /// Packed draw color including vanish alpha (InGameUI.cpp:5107-5112).
    pub fn floating_text_draw_rgba(color: (u8, u8, u8), alpha: u8) -> [f32; 4] {
        [
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
            alpha as f32 / 255.0,
        ]
    }
}
