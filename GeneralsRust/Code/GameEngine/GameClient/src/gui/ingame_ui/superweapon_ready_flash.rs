//! C++ InGameUI.cpp:3654-3666 READY strip flash (not static yellow 0:00).

/// C++ `m_superweaponUsedFlashColor` / last-flash-frame pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperweaponFlashState {
    pub used_flash_color: bool,
    pub last_flash_frame: u32,
}

/// C++ drawName/drawTime color 0 → default white.
pub const SUPERWEAPON_NORMAL_RGBA: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Previous live READY yellow `[1, 1, 0.2]` — flash stand-in vs white.
pub const SUPERWEAPON_FLASH_RGBA: [f32; 4] = [1.0, 1.0, 0.2, 1.0];
pub const SUPERWEAPON_NORMAL_POINT_SIZE: f32 = 10.0;
/// Ready-font size stand-in (`m_superweaponReadyPointSize` / bold).
pub const SUPERWEAPON_READY_POINT_SIZE: f32 = 12.0;

/// C++ InGameUI.cpp:3654-3666 — READY blinks flash color vs default.
pub fn superweapon_ready_draw_style(
    frame: u32,
    ready: bool,
    flash_duration: u32,
    flash_rgba: [f32; 4],
    state: &mut SuperweaponFlashState,
) -> ([f32; 4], f32) {
    if !ready {
        return (SUPERWEAPON_NORMAL_RGBA, SUPERWEAPON_NORMAL_POINT_SIZE);
    }
    if flash_duration != 0 && frame >= state.last_flash_frame.saturating_add(flash_duration) {
        state.used_flash_color = !state.used_flash_color;
        state.last_flash_frame = frame;
    }
    let color = if flash_duration == 0 || state.used_flash_color {
        SUPERWEAPON_NORMAL_RGBA
    } else {
        flash_rgba
    };
    (color, SUPERWEAPON_READY_POINT_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_strip_flashes_instead_of_static_yellow() {
        // Given: C++ flash duration 15 frames, usedFlashColor starts true.
        let mut state = SuperweaponFlashState {
            used_flash_color: true,
            last_flash_frame: 0,
        };

        // When: countdown is not ready
        let (idle, idle_size) =
            superweapon_ready_draw_style(1, false, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
        // Then: default white, normal font
        assert_eq!(idle, SUPERWEAPON_NORMAL_RGBA);
        assert_eq!(idle_size, SUPERWEAPON_NORMAL_POINT_SIZE);

        // When: READY at frame 1 (toggle window)
        let (ready0, ready_size) =
            superweapon_ready_draw_style(1, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
        // Then: ready font, default color (not static yellow)
        assert_eq!(ready_size, SUPERWEAPON_READY_POINT_SIZE);
        assert_eq!(ready0, SUPERWEAPON_NORMAL_RGBA);
        assert_ne!(ready0, SUPERWEAPON_FLASH_RGBA);

        // When: READY after flash duration
        let (ready1, _) =
            superweapon_ready_draw_style(16, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
        // Then: flash color (blink)
        assert_eq!(ready1, SUPERWEAPON_FLASH_RGBA);

        // When: READY after another duration
        let (ready2, _) =
            superweapon_ready_draw_style(31, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
        // Then: back to default
        assert_eq!(ready2, SUPERWEAPON_NORMAL_RGBA);
    }
}
