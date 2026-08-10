// Floating combat/resource text.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn add_floating_text(&mut self, text: String, position: Coord3D, color: (u8, u8, u8)) {
        if self.floating_texts.len() >= MAX_FLOATING_TEXT {
            self.floating_texts.remove(0);
        }
        self.floating_texts.push(FloatingTextData {
            text,
            position,
            color,
            creation_frame: self.current_frame,
            timeout: DEFAULT_FLOATING_TEXT_TIMEOUT,
            move_up_speed: 1.0,
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
            self.floating_texts.push(FloatingTextData {
                text: text.clone(),
                position: *position,
                color: *color,
                creation_frame: *creation_frame,
                timeout: *timeout,
                move_up_speed: self.floating_text_move_up_speed,
            });
        }
    }

    pub fn update_floating_texts(&mut self) {
        self.floating_texts
            .retain(|ft| self.current_frame - ft.creation_frame < ft.timeout);
        for ft in &mut self.floating_texts {
            ft.position.z += ft.move_up_speed;
        }
    }

}
